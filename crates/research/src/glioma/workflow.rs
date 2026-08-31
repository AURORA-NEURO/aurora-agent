//! Adaptive workflow planning for autonomous preclinical glioma research.
//!
//! The stage engine provides a safe, fixed execution graph.  This module is the product layer on
//! top of it: it turns the graph into a resumable campaign plan, uses already-qualified local
//! outputs to choose the next executable branch, and makes abstention/repair decisions explicit.
//! It never treats a model score as a result and never dispatches a physical instrument itself;
//! the caller-owned [`super::super::glioma_engine::GliomaStageExecutor`] remains the only effectful
//! seam.

use super::super::glioma_engine::{
    compile_glioma_research, execute_glioma_research, GliomaExecutionReceipt, GliomaModality,
    GliomaPlanDisposition, GliomaResearchIntent, GliomaStageExecutor, GliomaStageKind,
    StageReadiness,
};
use super::evidence::{EvidenceDisposition, EvidenceQualification};
use super::experiment::{ExperimentDesign, ExperimentDisposition};
use super::mechanism::{MechanismDisposition, MechanismPortfolio};
use super::multimodal::{MultimodalDisposition, MultimodalQcReport};
use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveWorkflow1@1";

/// A campaign focus.  The planner closes over upstream dependencies, so selecting a later focus
/// never silently drops evidence or QC gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaWorkflowMode {
    EvidenceDiscovery,
    MechanismValidation,
    ExperimentCampaign,
    ReplicationCampaign,
    FullProgram,
}

impl GliomaWorkflowMode {
    fn terminal_stages(self) -> &'static [GliomaStageKind] {
        match self {
            Self::EvidenceDiscovery => &[
                GliomaStageKind::EvidenceSurveillance,
                GliomaStageKind::EvidenceCompilation,
            ],
            Self::MechanismValidation => &[GliomaStageKind::MechanismExploration],
            Self::ExperimentCampaign => &[GliomaStageKind::StatisticalInterpretation],
            Self::ReplicationCampaign => &[GliomaStageKind::FederationBenchmarking],
            Self::FullProgram => &[GliomaStageKind::FederationBenchmarking],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaWorkflowRequest {
    pub intent: GliomaResearchIntent,
    pub mode: GliomaWorkflowMode,
    /// Completed stages are a checkpoint cursor, not a claim that an unverified artifact exists.
    /// They must be accompanied by the corresponding qualified output below when downstream
    /// stages depend on them.
    pub completed_stages: BTreeSet<GliomaStageKind>,
    pub evidence: Option<EvidenceQualification>,
    pub qc_report: Option<MultimodalQcReport>,
    pub mechanism_portfolio: Option<MechanismPortfolio>,
    pub experiment_design: Option<ExperimentDesign>,
    pub max_parallelism: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowNodeDecision {
    Ready,
    Completed,
    Hold,
    Abstain,
    ApprovalRequired,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaWorkflowNode {
    pub node_id: String,
    pub stage_kind: GliomaStageKind,
    pub depends_on: Vec<String>,
    pub capability: String,
    pub input_schemas: Vec<String>,
    pub output_schema: String,
    pub modalities: BTreeSet<GliomaModality>,
    pub autonomy_tier: AutonomyTier,
    pub budget_units: u32,
    pub decision: WorkflowNodeDecision,
    pub rationale: Vec<String>,
    pub stop_conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaWorkflowBranch {
    pub branch_id: String,
    pub trigger: String,
    pub target_order: Vec<String>,
    pub action: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaWorkflowPlan {
    pub schema_version: String,
    pub feature_id: String,
    pub output_schema: String,
    pub workflow_id: String,
    pub research_id: String,
    pub study_id: String,
    pub objective: String,
    pub mode: GliomaWorkflowMode,
    pub nodes: Vec<GliomaWorkflowNode>,
    pub topological_order: Vec<String>,
    pub ready_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub abstain_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub branches: Vec<GliomaWorkflowBranch>,
    pub budget_units: u32,
    pub max_parallelism: u16,
    /// Digests of caller-supplied checkpoint outputs, ordered by stage id. Binding these into the
    /// plan prevents a resumed campaign from accidentally reusing a plan for a different local
    /// evidence/QC/mechanism/design object.
    pub checkpoint_digest_order: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaWorkflowExecution {
    pub plan: GliomaWorkflowPlan,
    pub receipt: GliomaExecutionReceipt,
    pub executed_branch_order: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaWorkflowError {
    #[error("workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("workflow plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("workflow digest failed: {0}")]
    Digest(String),
    #[error("workflow execution is not admitted: {0}")]
    NotAdmitted(String),
    #[error("workflow engine failed: {0}")]
    Engine(String),
}

fn capability(kind: GliomaStageKind) -> &'static str {
    match kind {
        GliomaStageKind::IntentNormalization => "normalize-and-bound-glioma-intent",
        GliomaStageKind::EvidenceSurveillance => "retrieve-and-rank-glioma-evidence",
        GliomaStageKind::EvidenceCompilation => "compile-claims-and-omissions",
        GliomaStageKind::MultimodalIngestionQc => "harmonize-modalities-and-quarantine-defects",
        GliomaStageKind::MolecularLandscape => "assemble-multimodal-molecular-landscape",
        GliomaStageKind::MechanismExploration => "rank-competing-mechanisms-and-discriminators",
        GliomaStageKind::ExperimentDesign => "allocate-power-aware-preclinical-experiment",
        GliomaStageKind::ProtocolSimulation => "simulate-protocol-and-resource-constraints",
        GliomaStageKind::InstrumentPreflight => "preflight-signed-instrument-execution",
        GliomaStageKind::ComputationalExecution => "execute-reproducible-local-computation",
        GliomaStageKind::StatisticalInterpretation => "analyze-effects-and-null-results",
        GliomaStageKind::ReplicationRobustness => {
            "assess-cross-study-replication-and-heterogeneity"
        }
        GliomaStageKind::ResearchObjectRelease => "assemble-reproducibility-research-object",
        GliomaStageKind::FederationBenchmarking => "benchmark-permitted-aggregate-exports",
    }
}

fn scope_for_mode(
    mode: GliomaWorkflowMode,
    stages: &[super::super::glioma_engine::GliomaStage],
) -> BTreeSet<GliomaStageKind> {
    let mut scope = BTreeSet::from([GliomaStageKind::IntentNormalization]);
    let by_id = stages
        .iter()
        .map(|stage| (stage.stage_id.as_str(), stage))
        .collect::<BTreeMap<_, _>>();
    let mut pending = mode.terminal_stages().to_vec();
    while let Some(kind) = pending.pop() {
        if !scope.insert(kind) {
            continue;
        }
        if let Some(stage) = stages.iter().find(|stage| stage.kind == kind) {
            for dependency in &stage.depends_on {
                if let Some(parent) = by_id.get(dependency.as_str()) {
                    pending.push(parent.kind);
                }
            }
        }
    }
    scope
}

fn digest_input(plan: &GliomaWorkflowPlan) -> serde_json::Value {
    serde_json::json!({
        "schema_version": plan.schema_version,
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "workflow_id": plan.workflow_id,
        "research_id": plan.research_id,
        "study_id": plan.study_id,
        "objective": plan.objective,
        "mode": plan.mode,
        "nodes": plan.nodes,
        "topological_order": plan.topological_order,
        "ready_order": plan.ready_order,
        "completed_order": plan.completed_order,
        "hold_order": plan.hold_order,
        "abstain_order": plan.abstain_order,
        "approval_order": plan.approval_order,
        "skipped_order": plan.skipped_order,
        "branches": plan.branches,
        "budget_units": plan.budget_units,
        "max_parallelism": plan.max_parallelism,
        "checkpoint_digest_order": plan.checkpoint_digest_order,
        "replay_identity": plan.replay_identity,
        "boundary": plan.boundary,
    })
}

impl GliomaWorkflowPlan {
    pub fn validate(&self) -> Result<(), GliomaWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.workflow_id.trim().is_empty()
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.nodes.is_empty()
            || self.budget_units == 0
            || self.max_parallelism == 0
            || self
                .checkpoint_digest_order
                .iter()
                .any(|digest| digest.as_str().len() != 64)
            || self.replay_identity.as_str().len() != 64
        {
            return Err(GliomaWorkflowError::InvalidPlan(
                "identity, boundary, objective, node graph, budget, or replay is invalid".into(),
            ));
        }
        let ids = self
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<BTreeSet<_>>();
        if ids.len() != self.nodes.len()
            || self
                .topological_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != ids
            || self.topological_order.len() != ids.len()
            || self.nodes.iter().any(|node| {
                node.node_id.trim().is_empty()
                    || node.capability.trim().is_empty()
                    || node.output_schema.trim().is_empty()
                    || node
                        .depends_on
                        .iter()
                        .any(|dependency| !ids.contains(dependency))
                    || node.rationale.iter().any(|reason| reason.trim().is_empty())
                    || node
                        .stop_conditions
                        .iter()
                        .any(|condition| condition.trim().is_empty())
            })
        {
            return Err(GliomaWorkflowError::InvalidPlan(
                "node identities, dependencies, or typed contracts do not reconcile".into(),
            ));
        }
        let positions = self
            .topological_order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect::<BTreeMap<_, _>>();
        if self.nodes.iter().any(|node| {
            node.depends_on.iter().any(|dependency| {
                positions[dependency.as_str()] >= positions[node.node_id.as_str()]
            })
        }) {
            return Err(GliomaWorkflowError::InvalidPlan(
                "workflow graph is not topologically ordered".into(),
            ));
        }
        let canonical = |values: &[String]| values.windows(2).all(|pair| pair[0] <= pair[1]);
        if !canonical(&self.completed_order)
            || !canonical(&self.hold_order)
            || !canonical(&self.abstain_order)
            || !canonical(&self.approval_order)
            || !canonical(&self.skipped_order)
            || self
                .branches
                .windows(2)
                .any(|pair| pair[0].branch_id > pair[1].branch_id)
        {
            return Err(GliomaWorkflowError::InvalidPlan(
                "workflow status and branch ordering is not canonical".into(),
            ));
        }
        let decision_ids = |decision: WorkflowNodeDecision| {
            self.nodes
                .iter()
                .filter(|node| node.decision == decision)
                .map(|node| node.node_id.clone())
                .collect::<BTreeSet<_>>()
        };
        for (label, values, decision) in [
            ("ready", &self.ready_order, WorkflowNodeDecision::Ready),
            (
                "completed",
                &self.completed_order,
                WorkflowNodeDecision::Completed,
            ),
            ("hold", &self.hold_order, WorkflowNodeDecision::Hold),
            (
                "abstain",
                &self.abstain_order,
                WorkflowNodeDecision::Abstain,
            ),
            (
                "approval",
                &self.approval_order,
                WorkflowNodeDecision::ApprovalRequired,
            ),
            (
                "skipped",
                &self.skipped_order,
                WorkflowNodeDecision::Skipped,
            ),
        ] {
            if values.iter().cloned().collect::<BTreeSet<_>>() != decision_ids(decision) {
                return Err(GliomaWorkflowError::InvalidPlan(format!(
                    "{label} order does not reconcile with node decisions"
                )));
            }
        }
        if self.branches.iter().any(|branch| {
            branch.branch_id.trim().is_empty()
                || branch.trigger.trim().is_empty()
                || branch.action.trim().is_empty()
                || branch.rationale.trim().is_empty()
                || branch
                    .target_order
                    .iter()
                    .any(|target| !ids.contains(target))
        }) {
            return Err(GliomaWorkflowError::InvalidPlan(
                "branch identity, rationale, or target is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaWorkflowError::InvalidPlan(
                "workflow digest is not bound to its graph".into(),
            ));
        }
        Ok(())
    }

    /// Return the deterministic first wave of runnable nodes.  A host can call this after each
    /// checkpoint and safely submit at most `max_parallelism` independent local jobs.
    pub fn next_ready_batch(&self) -> Vec<String> {
        let completed = self.completed_order.iter().collect::<BTreeSet<_>>();
        self.topological_order
            .iter()
            .filter(|id| self.ready_order.contains(id))
            .filter(|id| {
                self.nodes
                    .iter()
                    .find(|node| &node.node_id == *id)
                    .map(|node| {
                        node.depends_on
                            .iter()
                            .all(|dependency| completed.contains(dependency))
                    })
                    .unwrap_or(false)
            })
            .take(self.max_parallelism as usize)
            .cloned()
            .collect()
    }
}

fn feedback_decision(
    kind: GliomaStageKind,
    request: &GliomaWorkflowRequest,
) -> Option<(WorkflowNodeDecision, String, String)> {
    match kind {
        GliomaStageKind::EvidenceCompilation | GliomaStageKind::MechanismExploration
            if request
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.disposition == EvidenceDisposition::Unresolved) =>
        {
            Some((WorkflowNodeDecision::Hold, "evidence-remediation-required".into(), "retain unknown and contradictory evidence before mechanism claims".into()))
        }
        GliomaStageKind::MolecularLandscape | GliomaStageKind::MechanismExploration
            if request
                .qc_report
                .as_ref()
                .is_some_and(|report| report.disposition != MultimodalDisposition::Qualified) =>
        {
            Some((WorkflowNodeDecision::Hold, "qc-remediation-required".into(), "repair or add an orthogonal modality before interpreting molecular structure".into()))
        }
        GliomaStageKind::ExperimentDesign
        | GliomaStageKind::ProtocolSimulation
        | GliomaStageKind::InstrumentPreflight
        | GliomaStageKind::ComputationalExecution
            if request
                .mechanism_portfolio
                .as_ref()
                .is_some_and(|portfolio| portfolio.disposition == MechanismDisposition::Unresolved) =>
        {
            Some((WorkflowNodeDecision::Hold, "mechanism-discrimination-required".into(), "run a discriminating observation instead of executing an unresolved mechanism".into()))
        }
        GliomaStageKind::InstrumentPreflight | GliomaStageKind::ComputationalExecution
            if request
                .experiment_design
                .as_ref()
                .is_some_and(|design| design.disposition != ExperimentDisposition::Ready) =>
        {
            Some((WorkflowNodeDecision::Hold, "power-repair-required".into(), "do not execute an underpowered or blocked design; repair allocation or explicitly release its null result".into()))
        }
        _ => None,
    }
}

fn context_completed(kind: GliomaStageKind, request: &GliomaWorkflowRequest) -> bool {
    match kind {
        GliomaStageKind::EvidenceCompilation => request.evidence.is_some(),
        GliomaStageKind::MultimodalIngestionQc => request.qc_report.is_some(),
        GliomaStageKind::MechanismExploration => request.mechanism_portfolio.is_some(),
        GliomaStageKind::ExperimentDesign => request.experiment_design.is_some(),
        _ => false,
    }
}

fn checkpoint_digest_order(request: &GliomaWorkflowRequest) -> Vec<ContentHash> {
    let mut digests = [
        (
            GliomaStageKind::EvidenceCompilation.stage_id(),
            request.evidence.as_ref().map(|value| value.digest.clone()),
        ),
        (
            GliomaStageKind::MultimodalIngestionQc.stage_id(),
            request.qc_report.as_ref().map(|value| value.digest.clone()),
        ),
        (
            GliomaStageKind::MechanismExploration.stage_id(),
            request
                .mechanism_portfolio
                .as_ref()
                .map(|value| value.digest.clone()),
        ),
        (
            GliomaStageKind::ExperimentDesign.stage_id(),
            request
                .experiment_design
                .as_ref()
                .map(|value| value.digest.clone()),
        ),
    ]
    .into_iter()
    .filter_map(|(stage_id, digest)| digest.map(|digest| (stage_id, digest)))
    .collect::<Vec<_>>();
    digests.sort_by(|left, right| left.0.cmp(right.0));
    digests.into_iter().map(|(_, digest)| digest).collect()
}

/// Compile an adaptive, checkpoint-oriented campaign from the fixed stage engine.
pub fn plan_glioma_workflow(
    request: &GliomaWorkflowRequest,
) -> Result<GliomaWorkflowPlan, GliomaWorkflowError> {
    if request.max_parallelism == 0 {
        return Err(GliomaWorkflowError::InvalidRequest(
            "max_parallelism must be positive".into(),
        ));
    }
    let base = compile_glioma_research(&request.intent)
        .map_err(|error| GliomaWorkflowError::InvalidRequest(error.to_string()))?;
    let known = GliomaStageKind::ALL.into_iter().collect::<BTreeSet<_>>();
    if request
        .completed_stages
        .iter()
        .any(|kind| !known.contains(kind))
    {
        return Err(GliomaWorkflowError::InvalidRequest(
            "checkpoint names an unknown stage".into(),
        ));
    }
    if request
        .evidence
        .as_ref()
        .is_some_and(|evidence| evidence.validate().is_err())
        || request
            .qc_report
            .as_ref()
            .is_some_and(|report| report.validate().is_err())
        || request
            .mechanism_portfolio
            .as_ref()
            .is_some_and(|portfolio| portfolio.validate().is_err())
        || request
            .experiment_design
            .as_ref()
            .is_some_and(|design| design.validate().is_err())
    {
        return Err(GliomaWorkflowError::InvalidRequest(
            "supplied checkpoint output failed its contract validation".into(),
        ));
    }
    for (kind, supplied) in [
        (
            GliomaStageKind::EvidenceCompilation,
            request.evidence.is_some(),
        ),
        (
            GliomaStageKind::MultimodalIngestionQc,
            request.qc_report.is_some(),
        ),
        (
            GliomaStageKind::MechanismExploration,
            request.mechanism_portfolio.is_some(),
        ),
        (
            GliomaStageKind::ExperimentDesign,
            request.experiment_design.is_some(),
        ),
    ] {
        if request.completed_stages.contains(&kind) && !supplied {
            return Err(GliomaWorkflowError::InvalidRequest(format!(
                "checkpoint marks {} completed without its typed output",
                kind.stage_id()
            )));
        }
    }
    let scope = scope_for_mode(request.mode, &base.stages);
    let stage_by_kind = base
        .stages
        .iter()
        .map(|stage| (stage.kind, stage))
        .collect::<BTreeMap<_, _>>();
    let mut nodes = Vec::with_capacity(base.stages.len());
    let mut ready = Vec::new();
    let mut completed = Vec::new();
    let mut hold = Vec::new();
    let mut abstain = Vec::new();
    let mut approval = Vec::new();
    let mut skipped = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut branches = Vec::new();
    let request_blocked = base.disposition == GliomaPlanDisposition::Blocked;

    for kind in GliomaStageKind::ALL {
        let stage = stage_by_kind[&kind];
        let node_id = stage.stage_id.clone();
        let mut rationale = Vec::new();
        let mut stop_conditions = vec![
            "never infer a biological observation from a missing or simulated artifact".into(),
        ];
        let decision = if !scope.contains(&kind) || !stage.required {
            skipped.push(node_id.clone());
            omissions.insert(format!("{node_id}:out-of-scope-or-not-requested"));
            WorkflowNodeDecision::Skipped
        } else if request_blocked {
            abstain.push(node_id.clone());
            omissions.insert(format!("{node_id}:request-blocked"));
            rationale.push(
                "the compiled intent violates a locality or aggregate-export invariant".into(),
            );
            WorkflowNodeDecision::Abstain
        } else if stage.readiness == StageReadiness::Disabled {
            skipped.push(node_id.clone());
            omissions.insert(format!("{node_id}:disabled-by-intent"));
            WorkflowNodeDecision::Skipped
        } else if request.completed_stages.contains(&kind) || context_completed(kind, request) {
            completed.push(node_id.clone());
            rationale.push(
                "checkpoint supplied by caller; downstream consumers must validate its artifact"
                    .into(),
            );
            WorkflowNodeDecision::Completed
        } else if let Some((decision, branch, reason)) = feedback_decision(kind, request) {
            rationale.push(reason.clone());
            hold.push(node_id.clone());
            omissions.insert(format!("{node_id}:{branch}"));
            branches.push(GliomaWorkflowBranch {
                branch_id: branch.clone(),
                trigger: format!("{node_id}:feedback-gate"),
                target_order: vec![node_id.clone()],
                action: "pause-and-request-a-bounded-remediation-or-new-observation".into(),
                rationale: reason,
            });
            decision
        } else {
            match stage.readiness {
                StageReadiness::Ready => {
                    ready.push(node_id.clone());
                    rationale.push("all static intent and dependency inputs are present".into());
                    WorkflowNodeDecision::Ready
                }
                StageReadiness::MissingInput => {
                    hold.push(node_id.clone());
                    omissions.insert(format!("{node_id}:required-input-missing"));
                    rationale.push("caller must provide a typed local artifact or modality".into());
                    WorkflowNodeDecision::Hold
                }
                StageReadiness::ApprovalRequired => {
                    approval.push(node_id.clone());
                    omissions.insert(format!("{node_id}:approval-missing"));
                    rationale
                        .push("authority is required before this effect can be admitted".into());
                    WorkflowNodeDecision::ApprovalRequired
                }
                StageReadiness::Disabled => unreachable!("handled above"),
            }
        };
        if matches!(
            decision,
            WorkflowNodeDecision::Ready | WorkflowNodeDecision::Completed
        ) {
            stop_conditions
                .push("stop on contradictory, poisoned, revoked, or policy-denied input".into());
        }
        nodes.push(GliomaWorkflowNode {
            node_id: node_id.clone(),
            stage_kind: kind,
            depends_on: stage.depends_on.clone(),
            capability: capability(kind).into(),
            input_schemas: stage.input_schemas.clone(),
            output_schema: stage.output_schema.clone(),
            modalities: request.intent.modalities.clone(),
            autonomy_tier: stage.autonomy_tier,
            budget_units: stage.budget_units,
            decision,
            rationale,
            stop_conditions,
        });
    }

    if request
        .evidence
        .as_ref()
        .is_some_and(|evidence| !evidence.negative_order.is_empty())
    {
        branches.push(GliomaWorkflowBranch {
            branch_id: "negative-evidence-preserved".into(),
            trigger: "evidence:negative-order-nonempty".into(),
            target_order: vec![GliomaStageKind::MechanismExploration.stage_id().into()],
            action: "carry negative evidence into competing-mechanism ranking".into(),
            rationale: "a negative or null result changes the next action but never disappears from the campaign".into(),
        });
    }
    if request
        .qc_report
        .as_ref()
        .is_some_and(|report| report.disposition != MultimodalDisposition::Qualified)
    {
        branches.push(GliomaWorkflowBranch {
            branch_id: "qc-repair-or-orthogonal-modality".into(),
            trigger: "multimodal-qc:not-qualified".into(),
            target_order: vec![GliomaStageKind::MultimodalIngestionQc.stage_id().into()],
            action: "quarantine defective observations and acquire a bounded orthogonal modality"
                .into(),
            rationale: "the planner refuses to manufacture comparability through imputation".into(),
        });
    }
    if request
        .mechanism_portfolio
        .as_ref()
        .is_some_and(|portfolio| !portfolio.contradicted_order.is_empty())
    {
        branches.push(GliomaWorkflowBranch {
            branch_id: "contradiction-resolution".into(),
            trigger: "mechanism:contradicted-order-nonempty".into(),
            target_order: vec![GliomaStageKind::MechanismExploration.stage_id().into()],
            action: "prioritize discriminating observations and retain the contradicted mechanism".into(),
            rationale: "contradictions are a research output and a routing signal, not a reason to silently delete a candidate".into(),
        });
    }
    branches.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    branches.dedup_by(|left, right| left.branch_id == right.branch_id);
    ready.sort();
    completed.sort();
    hold.sort();
    abstain.sort();
    approval.sort();
    skipped.sort();
    let budget_units = nodes
        .iter()
        .filter(|node| {
            !matches!(
                node.decision,
                WorkflowNodeDecision::Skipped | WorkflowNodeDecision::Completed
            )
        })
        .map(|node| node.budget_units)
        .sum();
    if budget_units > request.intent.budget_units {
        for node in &mut nodes {
            if node.decision == WorkflowNodeDecision::Ready {
                node.decision = WorkflowNodeDecision::Abstain;
                abstain.push(node.node_id.clone());
                ready.retain(|id| id != &node.node_id);
                omissions.insert(format!("{}:budget-exhausted", node.node_id));
            }
        }
        abstain.sort();
        branches.push(GliomaWorkflowBranch {
            branch_id: "budget-termination".into(),
            trigger: "workflow:planned-budget-below-required-cost".into(),
            target_order: Vec::new(),
            action: "terminate before side effects and request a smaller bounded campaign".into(),
            rationale: "resource exhaustion is an honest stop, never a degraded release".into(),
        });
        branches.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    }
    let mut plan = GliomaWorkflowPlan {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        workflow_id: format!(
            "glioma:{}:adaptive:{}",
            request.intent.research_id,
            format_mode(request.mode)
        ),
        research_id: request.intent.research_id.clone(),
        study_id: request.intent.study_id.clone(),
        objective: request.intent.objective.clone(),
        mode: request.mode,
        nodes,
        topological_order: base.stage_order.clone(),
        ready_order: ready,
        completed_order: completed,
        hold_order: hold,
        abstain_order: abstain,
        approval_order: approval,
        skipped_order: skipped,
        branches,
        budget_units,
        max_parallelism: request.max_parallelism,
        checkpoint_digest_order: checkpoint_digest_order(request),
        replay_identity: request.intent.replay_identity.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-workflow"),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| GliomaWorkflowError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

fn format_mode(mode: GliomaWorkflowMode) -> &'static str {
    match mode {
        GliomaWorkflowMode::EvidenceDiscovery => "evidence",
        GliomaWorkflowMode::MechanismValidation => "mechanism",
        GliomaWorkflowMode::ExperimentCampaign => "experiment",
        GliomaWorkflowMode::ReplicationCampaign => "replication",
        GliomaWorkflowMode::FullProgram => "full",
    }
}

/// Execute a fully-admitted adaptive campaign through the existing provider seam.  The planner
/// is evaluated first; a hold, approval gate, abstention, or budget stop prevents any provider
/// invocation.  This is deliberately a fresh-run API until a host supplies a checkpoint-aware
/// executor; callers can still use `completed_stages` to inspect and route resumptions.
pub fn execute_glioma_workflow<E: GliomaStageExecutor>(
    request: &GliomaWorkflowRequest,
    run_id: impl Into<String>,
    executor: &mut E,
) -> Result<GliomaWorkflowExecution, GliomaWorkflowError> {
    if !request.completed_stages.is_empty() {
        return Err(GliomaWorkflowError::NotAdmitted(
            "checkpointed campaigns require a host executor that proves idempotent replay; use plan_glioma_workflow for the next batch".into(),
        ));
    }
    let plan = plan_glioma_workflow(request)?;
    if request.mode != GliomaWorkflowMode::FullProgram {
        return Err(GliomaWorkflowError::NotAdmitted(
            "mode-scoped plans are execution cursors; use FullProgram when submitting the complete fixed stage graph to an executor".into(),
        ));
    }
    if !plan.hold_order.is_empty()
        || !plan.abstain_order.is_empty()
        || !plan.approval_order.is_empty()
    {
        return Err(GliomaWorkflowError::NotAdmitted(format!(
            "workflow has hold={}, abstain={}, approval={} gates",
            plan.hold_order.len(),
            plan.abstain_order.len(),
            plan.approval_order.len()
        )));
    }
    let base_plan = compile_glioma_research(&request.intent)
        .map_err(|error| GliomaWorkflowError::Engine(error.to_string()))?;
    let receipt = execute_glioma_research(
        &base_plan,
        run_id,
        executor,
        request.intent.max_retries,
        request.intent.input_artifacts.clone(),
    )
    .map_err(|error| GliomaWorkflowError::Engine(error.to_string()))?;
    let executed_branch_order = plan
        .branches
        .iter()
        .filter(|branch| {
            branch
                .target_order
                .iter()
                .any(|stage| receipt.completed_order.contains(stage))
        })
        .map(|branch| branch.branch_id.clone())
        .collect();
    Ok(GliomaWorkflowExecution {
        plan,
        receipt,
        executed_branch_order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{qualify_evidence, EvidenceRequest};
    use crate::glioma_engine::{DryRunGliomaExecutor, GliomaModelSystem, LocalArtifactRef};
    use bioprism_onco::OutputUse;

    fn intent() -> GliomaResearchIntent {
        GliomaResearchIntent {
            research_id: "glioma-workflow-test".into(),
            study_id: "study-001".into(),
            objective: "test preclinical invasion mechanism".into(),
            output_uses: BTreeSet::from([OutputUse::MethodDevelopment]),
            model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            modalities: BTreeSet::from([
                GliomaModality::Literature,
                GliomaModality::Transcriptomics,
                GliomaModality::Computational,
            ]),
            input_artifacts: vec![LocalArtifactRef {
                artifact_id: "local:matrix".into(),
                content_hash: ContentHash::of_bytes(b"matrix"),
                content_type: "application/octet-stream".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 256,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: ContentHash::of_bytes(b"workflow-replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn planner_is_deterministic_and_exposes_next_batch() {
        let request = GliomaWorkflowRequest {
            intent: intent(),
            mode: GliomaWorkflowMode::FullProgram,
            completed_stages: BTreeSet::new(),
            evidence: None,
            qc_report: None,
            mechanism_portfolio: None,
            experiment_design: None,
            max_parallelism: 3,
        };
        let first = plan_glioma_workflow(&request).unwrap();
        let second = plan_glioma_workflow(&request).unwrap();
        assert_eq!(first, second);
        first.validate().unwrap();
        assert_eq!(first.next_ready_batch(), vec!["intent-normalization"]);
        assert!(first.nodes.iter().any(|node| node.stage_kind
            == GliomaStageKind::InstrumentPreflight
            && node.decision == WorkflowNodeDecision::Skipped));
    }

    #[test]
    fn unresolved_evidence_holds_mechanisms_and_execution_never_starts() {
        let evidence = qualify_evidence(
            &EvidenceRequest {
                objective: "test preclinical invasion mechanism".into(),
                required_modalities: BTreeSet::from([GliomaModality::Literature]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                max_records: 4,
                min_quality_milli: 700,
                min_reproducibility_milli: 700,
            },
            &[],
        )
        .unwrap();
        assert_eq!(evidence.disposition, EvidenceDisposition::Unresolved);
        let request = GliomaWorkflowRequest {
            intent: intent(),
            mode: GliomaWorkflowMode::FullProgram,
            completed_stages: BTreeSet::new(),
            evidence: Some(evidence),
            qc_report: None,
            mechanism_portfolio: None,
            experiment_design: None,
            max_parallelism: 2,
        };
        let plan = plan_glioma_workflow(&request).unwrap();
        assert!(plan
            .completed_order
            .contains(&"evidence-compilation".into()));
        assert!(plan.hold_order.contains(&"mechanism-exploration".into()));
        let mut executor = DryRunGliomaExecutor;
        assert!(matches!(
            execute_glioma_workflow(&request, "glioma-run:blocked", &mut executor),
            Err(GliomaWorkflowError::NotAdmitted(_))
        ));
    }

    #[test]
    fn admitted_workflow_executes_only_through_caller_executor() {
        let request = GliomaWorkflowRequest {
            intent: intent(),
            mode: GliomaWorkflowMode::FullProgram,
            completed_stages: BTreeSet::new(),
            evidence: None,
            qc_report: None,
            mechanism_portfolio: None,
            experiment_design: None,
            max_parallelism: 2,
        };
        let mut executor = DryRunGliomaExecutor;
        let result = execute_glioma_workflow(&request, "glioma-run:workflow-test", &mut executor);
        assert!(
            result.is_ok(),
            "evidence workflow should be admitted: {result:?}"
        );
        let execution = result.unwrap();
        assert_eq!(execution.plan.mode, GliomaWorkflowMode::FullProgram);
        assert_eq!(execution.receipt.boundary, PRECLINICAL_BOUNDARY);
    }

    #[test]
    fn aggregate_export_violation_abstains_instead_of_skipping_the_federation_gate() {
        let mut intent = intent();
        intent.allow_federation = true;
        intent.aggregate_only = false;
        let request = GliomaWorkflowRequest {
            intent,
            mode: GliomaWorkflowMode::FullProgram,
            completed_stages: BTreeSet::new(),
            evidence: None,
            qc_report: None,
            mechanism_portfolio: None,
            experiment_design: None,
            max_parallelism: 2,
        };
        let plan = plan_glioma_workflow(&request).unwrap();
        assert!(plan
            .abstain_order
            .contains(&"federation-benchmarking".into()));
        assert!(!plan
            .skipped_order
            .contains(&"federation-benchmarking".into()));
    }

    #[test]
    fn checkpoint_without_typed_output_is_rejected() {
        let request = GliomaWorkflowRequest {
            intent: intent(),
            mode: GliomaWorkflowMode::MechanismValidation,
            completed_stages: BTreeSet::from([GliomaStageKind::MechanismExploration]),
            evidence: None,
            qc_report: None,
            mechanism_portfolio: None,
            experiment_design: None,
            max_parallelism: 1,
        };
        assert!(matches!(
            plan_glioma_workflow(&request),
            Err(GliomaWorkflowError::InvalidRequest(_))
        ));
    }
}
