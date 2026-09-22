//! Admission control from multimodal readiness into bounded downstream glioma routes.
//!
//! This is the execution-facing seam after multimodal synchronization.  It does not execute a
//! route; it converts the selected branch into an explicit action admission plan with route
//! types, dependency order, degraded handling, approval stops, and honest termination gates.

use super::multimodal_workflow::{
    MultimodalKnowledgeWorkflow, MultimodalWorkflowBarrierKind, MultimodalWorkflowDisposition,
};
use super::workflow_compile::LocalWorkflowStep;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchWorkflowAdmission1@1";
pub const MAX_ACTIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAdmissionRequest {
    pub objective: String,
    pub synchronization: MultimodalKnowledgeWorkflow,
    pub steps: Vec<LocalWorkflowStep>,
    pub max_actions: usize,
    pub allow_degraded_branch: bool,
    pub require_local_data_boundary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchAdmissionRoute {
    EvidenceAcquisition,
    MultimodalQualityControl,
    KnowledgeAdjudication,
    KnowledgeMonitoring,
    NegativeResultPreservation,
    ReplicationPlanning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchAdmissionGate {
    Admit,
    AdmitDegraded,
    ApprovalRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmittedResearchAction {
    pub action_id: String,
    pub step_id: String,
    pub route: ResearchAdmissionRoute,
    pub gate: ResearchAdmissionGate,
    pub dependency_order: Vec<String>,
    pub expected_artifact_kind: String,
    pub local_data_boundary: String,
    pub stop_condition_order: Vec<String>,
    pub barrier_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchAdmissionDisposition {
    Admitted,
    Degraded,
    ApprovalRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkflowAdmission {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_synchronization_digest: ContentHash,
    pub selected_branch: Option<String>,
    pub action_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub degraded_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub parallel_waves: Vec<Vec<String>>,
    pub route_order: Vec<ResearchAdmissionRoute>,
    pub actions: Vec<AdmittedResearchAction>,
    pub omission_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ResearchAdmissionDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkflowAdmissionError {
    #[error("workflow admission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("workflow admission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("workflow admission digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn route_for_artifact(kind: &str) -> ResearchAdmissionRoute {
    match kind {
        "typed_evidence_artifact" => ResearchAdmissionRoute::EvidenceAcquisition,
        "harmonized_knowledge_artifact" => ResearchAdmissionRoute::MultimodalQualityControl,
        "adjudication_artifact" => ResearchAdmissionRoute::KnowledgeAdjudication,
        "monitor_snapshot" => ResearchAdmissionRoute::KnowledgeMonitoring,
        "negative_result_artifact" => ResearchAdmissionRoute::NegativeResultPreservation,
        "replication_artifact" => ResearchAdmissionRoute::ReplicationPlanning,
        _ => ResearchAdmissionRoute::KnowledgeMonitoring,
    }
}

fn digest_input(output: &ResearchWorkflowAdmission) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_synchronization_digest": output.source_synchronization_digest,
        "selected_branch": output.selected_branch,
        "action_order": output.action_order,
        "admitted_order": output.admitted_order,
        "degraded_order": output.degraded_order,
        "approval_order": output.approval_order,
        "blocked_order": output.blocked_order,
        "parallel_waves": output.parallel_waves,
        "route_order": output.route_order,
        "actions": output.actions,
        "omission_order": output.omission_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ResearchWorkflowAdmission {
    pub fn validate(&self) -> Result<(), WorkflowAdmissionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.action_order)
            || !canonical(&self.admitted_order)
            || !canonical(&self.degraded_order)
            || !canonical(&self.approval_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.route_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.uncertainty)
            || self.parallel_waves.iter().any(|wave| !canonical(wave))
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.step_id.trim().is_empty()
                    || action.expected_artifact_kind.trim().is_empty()
                    || action.local_data_boundary.trim().is_empty()
                    || !canonical(&action.dependency_order)
                    || !canonical(&action.stop_condition_order)
                    || !canonical(&action.barrier_order)
            })
        {
            return Err(WorkflowAdmissionError::InvalidOutput(
                "identity, ordering, wave, route, or action fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| WorkflowAdmissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(WorkflowAdmissionError::InvalidOutput(
                "workflow admission digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

pub fn admit_glioma_research_workflow(
    request: &WorkflowAdmissionRequest,
) -> Result<ResearchWorkflowAdmission, WorkflowAdmissionError> {
    if request.objective.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.steps.is_empty()
        || request.steps.len() > MAX_ACTIONS
    {
        return Err(WorkflowAdmissionError::InvalidRequest(
            "objective, bounded action capacity, and compiled workflow steps are required".into(),
        ));
    }
    request
        .synchronization
        .validate()
        .map_err(|error| WorkflowAdmissionError::InvalidRequest(error.to_string()))?;
    if request.synchronization.objective != request.objective {
        return Err(WorkflowAdmissionError::InvalidRequest(
            "admission objective must bind to synchronization objective".into(),
        ));
    }
    let action_order = request.synchronization.action_order.clone();
    let action_set = action_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for step in &request.steps {
        if !action_set.contains(&step.action_id) || !seen.insert(step.action_id.clone()) {
            return Err(WorkflowAdmissionError::InvalidRequest(
                "steps must be unique and bind to the synchronized workflow action order".into(),
            ));
        }
    }
    if seen.len() != action_set.len() {
        return Err(WorkflowAdmissionError::InvalidRequest(
            "every synchronized action must have an admission step".into(),
        ));
    }
    let action_by_id = request
        .steps
        .iter()
        .map(|step| (step.action_id.clone(), step))
        .collect::<std::collections::BTreeMap<_, _>>();
    let hard_barriers = request
        .synchronization
        .barriers
        .iter()
        .filter(|barrier| {
            matches!(
                barrier.kind,
                MultimodalWorkflowBarrierKind::NonExportableObservation
                    | MultimodalWorkflowBarrierKind::WorkflowIncomplete
                    | MultimodalWorkflowBarrierKind::UnalignedObservation
            )
        })
        .map(|barrier| barrier.barrier_id.clone())
        .collect::<BTreeSet<_>>();
    let branch = request.synchronization.selected_branch.clone();
    let base_gate = match request.synchronization.disposition {
        MultimodalWorkflowDisposition::Ready => ResearchAdmissionGate::Admit,
        MultimodalWorkflowDisposition::Degraded if request.allow_degraded_branch => {
            ResearchAdmissionGate::AdmitDegraded
        }
        MultimodalWorkflowDisposition::ApprovalRequired => ResearchAdmissionGate::ApprovalRequired,
        MultimodalWorkflowDisposition::Degraded | MultimodalWorkflowDisposition::Blocked => {
            ResearchAdmissionGate::Blocked
        }
    };
    let mut actions = Vec::new();
    let mut admitted_order = Vec::new();
    let mut degraded_order = Vec::new();
    let mut approval_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut omission_order = Vec::new();
    let mut uncertainty = Vec::new();
    let mut routes = BTreeSet::new();
    for action_id in &action_order {
        let step = action_by_id[action_id];
        let barriers = request
            .synchronization
            .barriers
            .iter()
            .filter(|barrier| barrier.action_id == *action_id || barrier.action_id == "__global__")
            .map(|barrier| barrier.barrier_id.clone())
            .collect::<Vec<_>>();
        let action_hard_blocked = barriers
            .iter()
            .any(|barrier| hard_barriers.contains(barrier));
        let gate = if action_hard_blocked {
            ResearchAdmissionGate::Blocked
        } else {
            base_gate
        };
        let route = route_for_artifact(&step.expected_artifact_kind);
        routes.insert(route);
        let mut stop_conditions = vec![
            "policy_denial".to_string(),
            "provenance_mismatch".to_string(),
            "budget_exhausted".to_string(),
            "negative_or_null_result".to_string(),
        ];
        if gate == ResearchAdmissionGate::AdmitDegraded {
            stop_conditions.push("coverage_below_declared_threshold".into());
        }
        stop_conditions.sort();
        let local_data_boundary = if request.require_local_data_boundary {
            "raw_artifacts_local_only_aggregate_exports_only".into()
        } else {
            "caller_policy_boundary_required_before_data_movement".into()
        };
        let action = AdmittedResearchAction {
            action_id: action_id.clone(),
            step_id: step.step_id.clone(),
            route,
            gate,
            dependency_order: step.dependency_order.clone(),
            expected_artifact_kind: step.expected_artifact_kind.clone(),
            local_data_boundary,
            stop_condition_order: stop_conditions,
            barrier_order: barriers.clone(),
        };
        match gate {
            ResearchAdmissionGate::Admit => admitted_order.push(action_id.clone()),
            ResearchAdmissionGate::AdmitDegraded => degraded_order.push(action_id.clone()),
            ResearchAdmissionGate::ApprovalRequired => approval_order.push(action_id.clone()),
            ResearchAdmissionGate::Blocked => blocked_order.push(action_id.clone()),
        }
        if !barriers.is_empty() {
            omission_order.extend(
                barriers
                    .iter()
                    .map(|barrier| format!("{action_id}:{barrier}")),
            );
            uncertainty.extend(
                barriers
                    .iter()
                    .map(|barrier| format!("{action_id}:{barrier}")),
            );
        }
        actions.push(action);
    }
    let mut parallel_waves = request
        .synchronization
        .branches
        .iter()
        .find(|candidate| candidate.branch_id == branch.clone().unwrap_or_default())
        .map(|_| {
            request
                .steps
                .iter()
                .fold(
                    std::collections::BTreeMap::<usize, Vec<String>>::new(),
                    |mut waves, step| {
                        waves
                            .entry(step.wave)
                            .or_default()
                            .push(step.action_id.clone());
                        waves
                    },
                )
                .into_values()
                .map(|mut wave| {
                    wave.sort();
                    wave
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    parallel_waves.retain(|wave| wave.iter().any(|action| !blocked_order.contains(action)));
    let disposition = if !approval_order.is_empty()
        && request.synchronization.disposition == MultimodalWorkflowDisposition::ApprovalRequired
    {
        ResearchAdmissionDisposition::ApprovalRequired
    } else if !admitted_order.is_empty() && blocked_order.is_empty() && degraded_order.is_empty() {
        ResearchAdmissionDisposition::Admitted
    } else if !degraded_order.is_empty() && blocked_order.is_empty() {
        ResearchAdmissionDisposition::Degraded
    } else {
        ResearchAdmissionDisposition::Blocked
    };
    if !request.require_local_data_boundary {
        uncertainty
            .push("caller must establish a local-data boundary before route execution".into());
    }
    omission_order.sort();
    omission_order.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let next_step = match disposition {
        ResearchAdmissionDisposition::Admitted => {
            "hand admitted dependency waves to the local dispatcher; stop on any declared gate"
        }
        ResearchAdmissionDisposition::Degraded => {
            "hand only the degraded branch to local computation and carry missingness into interpretation"
        }
        ResearchAdmissionDisposition::ApprovalRequired => {
            "obtain scoped authorization before admitting approval-gated research actions"
        }
        ResearchAdmissionDisposition::Blocked => {
            "resolve synchronization barriers and re-run multimodal workflow admission"
        }
    };
    let mut output = ResearchWorkflowAdmission {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_synchronization_digest: request.synchronization.digest.clone(),
        selected_branch: branch,
        action_order,
        admitted_order,
        degraded_order,
        approval_order,
        blocked_order,
        parallel_waves,
        route_order: routes.into_iter().collect(),
        actions,
        omission_order,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| WorkflowAdmissionError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| WorkflowAdmissionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::multimodal_workflow::{
        compile_multimodal_knowledge_workflow, ModalityWorkflowObservation,
        MultimodalWorkflowRequest,
    };
    use crate::glioma::programs::p02_evidence_knowledge::workflow_compile::{
        LocalResearchWorkflow, LocalWorkflowDisposition, LocalWorkflowStep,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn local_workflow(disposition: LocalWorkflowDisposition) -> LocalResearchWorkflow {
        let step = LocalWorkflowStep {
            step_id: "workflow-step:harmonize".into(),
            action_id: "harmonize".into(),
            claim_key: "egfr-invasion".into(),
            dependency_order: Vec::new(),
            wave: 0,
            retry_limit: 1,
            checkpoint_after: true,
            compensation_kind: "restore_pre_harmonized_state".into(),
            expected_artifact_kind: "harmonized_knowledge_artifact".into(),
        };
        let mut output = LocalResearchWorkflow {
            feature_id: super::super::workflow_compile::FEATURE_ID.into(),
            output_schema: super::super::workflow_compile::OUTPUT_SCHEMA.into(),
            objective: "glioma invasion".into(),
            source_plan_digest: ContentHash::of_value(&serde_json::json!({"plan": "test"}))
                .unwrap(),
            step_order: vec!["harmonize".into()],
            parallel_waves: vec![vec!["harmonize".into()]],
            checkpoint_order: vec!["harmonize".into()],
            compensation_order: vec!["harmonize".into()],
            steps: vec![step],
            critical_path_steps: 1,
            total_cost_milli: 100,
            omitted_order: Vec::new(),
            uncertainty: Vec::new(),
            disposition,
            next_step: "test".into(),
            digest: ContentHash::of_value(&serde_json::Value::Null).unwrap(),
        };
        output.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": output.feature_id,
            "output_schema": output.output_schema,
            "objective": output.objective,
            "source_plan_digest": output.source_plan_digest,
            "step_order": output.step_order,
            "parallel_waves": output.parallel_waves,
            "checkpoint_order": output.checkpoint_order,
            "compensation_order": output.compensation_order,
            "steps": output.steps,
            "critical_path_steps": output.critical_path_steps,
            "total_cost_milli": output.total_cost_milli,
            "omitted_order": output.omitted_order,
            "uncertainty": output.uncertainty,
            "disposition": output.disposition,
            "next_step": output.next_step,
        }))
        .unwrap();
        output
    }

    fn sync(
        disposition: LocalWorkflowDisposition,
        include_transcriptomics: bool,
    ) -> MultimodalKnowledgeWorkflow {
        let workflow = local_workflow(disposition);
        let mut observations = vec![ModalityWorkflowObservation {
            study_id: "study-a".into(),
            site_id: "site-a".into(),
            action_id: "harmonize".into(),
            modality: GliomaModality::Imaging,
            model_system: GliomaModelSystem::Organoid,
            readiness_milli: 950,
            missingness_milli: 20,
            alignment_milli: 940,
            semantic_consistency_milli: 930,
            exportable: true,
            preclinical_only: true,
        }];
        if include_transcriptomics {
            observations.push(ModalityWorkflowObservation {
                modality: GliomaModality::Transcriptomics,
                ..observations[0].clone()
            });
        }
        compile_multimodal_knowledge_workflow(&MultimodalWorkflowRequest {
            objective: "glioma invasion".into(),
            workflow,
            observations,
            required_modalities: BTreeSet::from([
                GliomaModality::Imaging,
                GliomaModality::Transcriptomics,
            ]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_studies: 1,
            min_modalities_per_study: 1,
            readiness_threshold_milli: 800,
            max_barriers: 32,
            allow_degraded_branch: true,
        })
        .unwrap()
    }

    #[test]
    fn admits_full_branch_into_typed_downstream_route() {
        let synchronization = sync(LocalWorkflowDisposition::Ready, true);
        let output = admit_glioma_research_workflow(&WorkflowAdmissionRequest {
            objective: "glioma invasion".into(),
            steps: synchronization
                .action_order
                .iter()
                .map(|action_id| LocalWorkflowStep {
                    step_id: format!("workflow-step:{action_id}"),
                    action_id: action_id.clone(),
                    claim_key: "egfr-invasion".into(),
                    dependency_order: Vec::new(),
                    wave: 0,
                    retry_limit: 1,
                    checkpoint_after: true,
                    compensation_kind: "restore_pre_harmonized_state".into(),
                    expected_artifact_kind: "harmonized_knowledge_artifact".into(),
                })
                .collect(),
            synchronization,
            max_actions: 4,
            allow_degraded_branch: true,
            require_local_data_boundary: true,
        })
        .expect("admission");
        assert_eq!(output.disposition, ResearchAdmissionDisposition::Admitted);
        assert_eq!(output.admitted_order, vec!["harmonize"]);
        assert_eq!(
            output.route_order,
            vec![ResearchAdmissionRoute::MultimodalQualityControl]
        );
        output.validate().expect("digest validates");
    }

    #[test]
    fn degraded_branch_is_admitted_with_an_explicit_stop_condition() {
        let synchronization = sync(LocalWorkflowDisposition::Ready, false);
        let output = admit_glioma_research_workflow(&WorkflowAdmissionRequest {
            objective: "glioma invasion".into(),
            steps: synchronization
                .action_order
                .iter()
                .map(|action_id| LocalWorkflowStep {
                    step_id: format!("workflow-step:{action_id}"),
                    action_id: action_id.clone(),
                    claim_key: "egfr-invasion".into(),
                    dependency_order: Vec::new(),
                    wave: 0,
                    retry_limit: 1,
                    checkpoint_after: true,
                    compensation_kind: "restore_pre_harmonized_state".into(),
                    expected_artifact_kind: "harmonized_knowledge_artifact".into(),
                })
                .collect(),
            synchronization,
            max_actions: 4,
            allow_degraded_branch: true,
            require_local_data_boundary: true,
        })
        .expect("admission");
        assert_eq!(output.disposition, ResearchAdmissionDisposition::Degraded);
        assert!(output.actions[0]
            .stop_condition_order
            .contains(&"coverage_below_declared_threshold".into()));
    }

    #[test]
    fn approval_required_workflow_never_becomes_admitted() {
        let synchronization = sync(LocalWorkflowDisposition::ApprovalRequired, true);
        let output = admit_glioma_research_workflow(&WorkflowAdmissionRequest {
            objective: "glioma invasion".into(),
            steps: synchronization
                .action_order
                .iter()
                .map(|action_id| LocalWorkflowStep {
                    step_id: format!("workflow-step:{action_id}"),
                    action_id: action_id.clone(),
                    claim_key: "egfr-invasion".into(),
                    dependency_order: Vec::new(),
                    wave: 0,
                    retry_limit: 1,
                    checkpoint_after: true,
                    compensation_kind: "restore_pre_harmonized_state".into(),
                    expected_artifact_kind: "harmonized_knowledge_artifact".into(),
                })
                .collect(),
            synchronization,
            max_actions: 4,
            allow_degraded_branch: true,
            require_local_data_boundary: true,
        })
        .expect("admission");
        assert_eq!(
            output.disposition,
            ResearchAdmissionDisposition::ApprovalRequired
        );
        assert!(output.admitted_order.is_empty());
        assert_eq!(output.approval_order, vec!["harmonize"]);
    }
}
