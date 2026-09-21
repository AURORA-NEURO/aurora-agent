//! Adaptive synchronization for multimodal, multi-study glioma workflows.
//!
//! The local workflow compiler produces a dependency-safe action DAG.  This feature is the
//! next planning layer: it checks whether each study can actually support that DAG across the
//! requested modalities and preclinical model systems, then chooses a full, degraded, or
//! acquire-coverage branch before any downstream computation or experiment execution starts.
//! It never moves raw data or executes an instrument.

use super::workflow_compile::{LocalResearchWorkflow, LocalWorkflowDisposition};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalKnowledgeWorkflow1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_BARRIERS: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkflowRequest {
    pub objective: String,
    pub workflow: LocalResearchWorkflow,
    pub observations: Vec<ModalityWorkflowObservation>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub min_studies: usize,
    pub min_modalities_per_study: usize,
    pub readiness_threshold_milli: u16,
    pub max_barriers: usize,
    pub allow_degraded_branch: bool,
}

/// A value-only readiness observation.  Specimen bytes stay in the institution-local store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityWorkflowObservation {
    pub study_id: String,
    pub site_id: String,
    pub action_id: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub readiness_milli: u16,
    pub missingness_milli: u16,
    pub alignment_milli: u16,
    pub semantic_consistency_milli: u16,
    pub exportable: bool,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalWorkflowBarrierKind {
    MissingModality,
    MissingModelSystem,
    LowReadiness,
    InsufficientStudies,
    InsufficientModalities,
    UnalignedObservation,
    NonExportableObservation,
    WorkflowApproval,
    WorkflowIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkflowBarrier {
    pub barrier_id: String,
    pub study_id: String,
    pub action_id: String,
    pub kind: MultimodalWorkflowBarrierKind,
    pub missing_modality: Option<GliomaModality>,
    pub missing_model_system: Option<GliomaModelSystem>,
    pub severity_milli: u16,
    pub rationale: String,
    pub route: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalStudyReadiness {
    pub study_id: String,
    pub site_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub coverage_milli: u16,
    pub quality_milli: u16,
    pub readiness_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalWorkflowBranchKind {
    FullMultimodal,
    DegradedCoverage,
    AcquireMissingCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkflowBranch {
    pub branch_id: String,
    pub kind: MultimodalWorkflowBranchKind,
    pub action_order: Vec<String>,
    pub study_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub support_milli: u16,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalWorkflowDisposition {
    Ready,
    Degraded,
    ApprovalRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalKnowledgeWorkflow {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_workflow_digest: ContentHash,
    pub study_order: Vec<String>,
    pub action_order: Vec<String>,
    pub study_readiness: Vec<MultimodalStudyReadiness>,
    pub barrier_order: Vec<String>,
    pub barriers: Vec<MultimodalWorkflowBarrier>,
    pub branch_order: Vec<String>,
    pub branches: Vec<MultimodalWorkflowBranch>,
    pub selected_branch: Option<String>,
    pub omission_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultimodalWorkflowDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalWorkflowError {
    #[error("multimodal workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal workflow digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn quality(observation: &ModalityWorkflowObservation) -> u16 {
    observation
        .readiness_milli
        .min(1_000_u16.saturating_sub(observation.missingness_milli))
        .min(observation.alignment_milli)
        .min(observation.semantic_consistency_milli)
}

fn digest_input(output: &MultimodalKnowledgeWorkflow) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_workflow_digest": output.source_workflow_digest,
        "study_order": output.study_order,
        "action_order": output.action_order,
        "study_readiness": output.study_readiness,
        "barrier_order": output.barrier_order,
        "barriers": output.barriers,
        "branch_order": output.branch_order,
        "branches": output.branches,
        "selected_branch": output.selected_branch,
        "omission_order": output.omission_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl MultimodalKnowledgeWorkflow {
    pub fn validate(&self) -> Result<(), MultimodalWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.study_order)
            || !canonical(&self.action_order)
            || !canonical(&self.barrier_order)
            || !canonical(&self.branch_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.uncertainty)
            || self.study_readiness.iter().any(|row| {
                row.study_id.trim().is_empty()
                    || !canonical(&row.site_order)
                    || !canonical(&row.modality_order)
                    || !canonical(&row.model_system_order)
                    || !canonical(&row.missing_modality_order)
                    || !canonical(&row.missing_model_system_order)
                    || row.coverage_milli > 1_000
                    || row.quality_milli > 1_000
                    || row.readiness_milli > 1_000
            })
            || self.barriers.iter().any(|barrier| {
                barrier.barrier_id.trim().is_empty()
                    || barrier.study_id.trim().is_empty()
                    || barrier.action_id.trim().is_empty()
                    || barrier.rationale.trim().is_empty()
                    || barrier.route.trim().is_empty()
                    || barrier.severity_milli > 1_000
            })
            || self.branches.iter().any(|branch| {
                branch.branch_id.trim().is_empty()
                    || !canonical(&branch.action_order)
                    || !canonical(&branch.study_order)
                    || !canonical(&branch.missing_modality_order)
                    || !canonical(&branch.missing_model_system_order)
                    || branch.support_milli > 1_000
            })
            || self
                .branches
                .iter()
                .filter(|branch| branch.selected)
                .count()
                > 1
            || self.selected_branch.as_ref().is_some_and(|selected| {
                !self
                    .branches
                    .iter()
                    .any(|branch| branch.selected && &branch.branch_id == selected)
            })
        {
            return Err(MultimodalWorkflowError::InvalidOutput(
                "identity, ordering, score, barrier, or branch fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalWorkflowError::InvalidOutput(
                "multimodal workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

pub fn compile_multimodal_knowledge_workflow(
    request: &MultimodalWorkflowRequest,
) -> Result<MultimodalKnowledgeWorkflow, MultimodalWorkflowError> {
    if request.objective.trim().is_empty()
        || request.min_studies == 0
        || request.min_modalities_per_study == 0
        || request.readiness_threshold_milli == 0
        || request.readiness_threshold_milli > 1_000
        || request.max_barriers == 0
        || request.max_barriers > MAX_BARRIERS
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.required_modalities.is_empty()
        || request.required_model_systems.is_empty()
    {
        return Err(MultimodalWorkflowError::InvalidRequest(
            "objective, workflow requirements, observations, and bounded readiness policy are required".into(),
        ));
    }
    request
        .workflow
        .validate()
        .map_err(|error| MultimodalWorkflowError::InvalidRequest(error.to_string()))?;
    if request.workflow.objective != request.objective {
        return Err(MultimodalWorkflowError::InvalidRequest(
            "multimodal workflow objective must bind to the local workflow objective".into(),
        ));
    }

    let action_set = request
        .workflow
        .step_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observation_keys = BTreeSet::new();
    for observation in &request.observations {
        if observation.study_id.trim().is_empty()
            || observation.site_id.trim().is_empty()
            || observation.action_id.trim().is_empty()
            || !action_set.contains(&observation.action_id)
            || observation.readiness_milli > 1_000
            || observation.missingness_milli > 1_000
            || observation.alignment_milli > 1_000
            || observation.semantic_consistency_milli > 1_000
        {
            return Err(MultimodalWorkflowError::InvalidRequest(
                "observations must bind to workflow actions, use non-empty identifiers, and keep scores in 0..=1000".into(),
            ));
        }
        if !observation.preclinical_only {
            return Err(MultimodalWorkflowError::InvalidRequest(
                "human-subject or non-preclinical observations are permanently out of scope".into(),
            ));
        }
        let key = (
            observation.study_id.clone(),
            observation.action_id.clone(),
            observation.modality,
            observation.model_system,
        );
        if !observation_keys.insert(key) {
            return Err(MultimodalWorkflowError::InvalidRequest(
                "duplicate study/action/modality/model observation".into(),
            ));
        }
    }

    let mut study_observations = BTreeMap::<String, Vec<&ModalityWorkflowObservation>>::new();
    for observation in &request.observations {
        study_observations
            .entry(observation.study_id.clone())
            .or_default()
            .push(observation);
    }
    let study_order = study_observations.keys().cloned().collect::<Vec<_>>();
    let mut barriers = Vec::new();
    let mut add_barrier = |barrier: MultimodalWorkflowBarrier| {
        barriers.push(barrier);
    };

    if study_order.len() < request.min_studies {
        add_barrier(MultimodalWorkflowBarrier {
            barrier_id: "global:insufficient-studies".into(),
            study_id: "__global__".into(),
            action_id: "__global__".into(),
            kind: MultimodalWorkflowBarrierKind::InsufficientStudies,
            missing_modality: None,
            missing_model_system: None,
            severity_milli: 1_000,
            rationale: format!(
                "{} studies are available but {} are required",
                study_order.len(),
                request.min_studies
            ),
            route: "acquire_or_bind_additional_preclinical_study".into(),
        });
    }
    let mut study_readiness = Vec::new();
    let mut missing_modalities = BTreeSet::new();
    let mut missing_models = BTreeSet::new();
    for (study_id, observations) in &study_observations {
        let mut sites = BTreeSet::new();
        let mut modalities = BTreeSet::new();
        let mut models = BTreeSet::new();
        let mut usable_modalities = BTreeSet::new();
        let mut usable_models = BTreeSet::new();
        let mut quality_by_pair = BTreeMap::<(GliomaModality, GliomaModelSystem), u16>::new();
        for observation in observations {
            sites.insert(observation.site_id.clone());
            modalities.insert(observation.modality);
            models.insert(observation.model_system);
            let score = quality(observation);
            quality_by_pair
                .entry((observation.modality, observation.model_system))
                .and_modify(|existing| *existing = (*existing).max(score))
                .or_insert(score);
            if !observation.exportable {
                add_barrier(MultimodalWorkflowBarrier {
                    barrier_id: format!("{study_id}:{}:non-exportable", observation.action_id),
                    study_id: study_id.clone(),
                    action_id: observation.action_id.clone(),
                    kind: MultimodalWorkflowBarrierKind::NonExportableObservation,
                    missing_modality: None,
                    missing_model_system: None,
                    severity_milli: 1_000,
                    rationale: "observation is not permitted for cross-study synchronization"
                        .into(),
                    route: "keep_branch_local_or_request_policy_approved_aggregate".into(),
                });
            } else if score >= request.readiness_threshold_milli {
                usable_modalities.insert(observation.modality);
                usable_models.insert(observation.model_system);
            } else {
                add_barrier(MultimodalWorkflowBarrier {
                    barrier_id: format!("{study_id}:{}:low-readiness", observation.action_id),
                    study_id: study_id.clone(),
                    action_id: observation.action_id.clone(),
                    kind: MultimodalWorkflowBarrierKind::LowReadiness,
                    missing_modality: None,
                    missing_model_system: None,
                    severity_milli: 1_000_u16.saturating_sub(score),
                    rationale: format!(
                        "best quality score {score} is below readiness threshold {}",
                        request.readiness_threshold_milli
                    ),
                    route: "route_to_quality_remediation_before_fusion".into(),
                });
            }
        }
        if modalities.len() < request.min_modalities_per_study {
            add_barrier(MultimodalWorkflowBarrier {
                barrier_id: format!("{study_id}:insufficient-modalities"),
                study_id: study_id.clone(),
                action_id: "__study__".into(),
                kind: MultimodalWorkflowBarrierKind::InsufficientModalities,
                missing_modality: None,
                missing_model_system: None,
                severity_milli: 900,
                rationale: format!(
                    "study exposes {} modality families but {} are required",
                    modalities.len(),
                    request.min_modalities_per_study
                ),
                route: "bind_or_acquire_additional_modality_coverage".into(),
            });
        }
        let mut missing_modality_order = request
            .required_modalities
            .difference(&usable_modalities)
            .copied()
            .collect::<Vec<_>>();
        let mut missing_model_system_order = request
            .required_model_systems
            .difference(&usable_models)
            .copied()
            .collect::<Vec<_>>();
        missing_modality_order.sort();
        missing_model_system_order.sort();
        for modality in &missing_modality_order {
            missing_modalities.insert(*modality);
            add_barrier(MultimodalWorkflowBarrier {
                barrier_id: format!("{study_id}:missing-modality:{modality:?}"),
                study_id: study_id.clone(),
                action_id: "__study__".into(),
                kind: MultimodalWorkflowBarrierKind::MissingModality,
                missing_modality: Some(*modality),
                missing_model_system: None,
                severity_milli: 900,
                rationale: format!("required modality {modality:?} is not readiness-qualified"),
                route: "acquire_or_repair_modality_coverage".into(),
            });
        }
        for model_system in &missing_model_system_order {
            missing_models.insert(*model_system);
            add_barrier(MultimodalWorkflowBarrier {
                barrier_id: format!("{study_id}:missing-model:{model_system:?}"),
                study_id: study_id.clone(),
                action_id: "__study__".into(),
                kind: MultimodalWorkflowBarrierKind::MissingModelSystem,
                missing_modality: None,
                missing_model_system: Some(*model_system),
                severity_milli: 900,
                rationale: format!(
                    "required preclinical model system {model_system:?} is not readiness-qualified"
                ),
                route: "acquire_or_bind_model_system_coverage".into(),
            });
        }
        let total_slots = request.required_modalities.len() + request.required_model_systems.len();
        let covered_slots = request.required_modalities.len() - missing_modality_order.len()
            + request.required_model_systems.len()
            - missing_model_system_order.len();
        let coverage_milli = ((covered_slots * 1_000) / total_slots).min(1_000) as u16;
        let quality_milli = if quality_by_pair.is_empty() {
            0
        } else {
            (quality_by_pair
                .values()
                .map(|value| u32::from(*value))
                .sum::<u32>()
                / quality_by_pair.len() as u32) as u16
        };
        let readiness_milli = coverage_milli.min(quality_milli);
        study_readiness.push(MultimodalStudyReadiness {
            study_id: study_id.clone(),
            site_order: sites.into_iter().collect(),
            modality_order: modalities.into_iter().collect(),
            model_system_order: models.into_iter().collect(),
            missing_modality_order,
            missing_model_system_order,
            coverage_milli,
            quality_milli,
            readiness_milli,
        });
    }

    for action_id in &request.workflow.step_order {
        if !request
            .observations
            .iter()
            .any(|observation| &observation.action_id == action_id)
        {
            add_barrier(MultimodalWorkflowBarrier {
                barrier_id: format!("action:{action_id}:unaligned"),
                study_id: "__global__".into(),
                action_id: action_id.clone(),
                kind: MultimodalWorkflowBarrierKind::UnalignedObservation,
                missing_modality: None,
                missing_model_system: None,
                severity_milli: 1_000,
                rationale: "workflow action has no bound multimodal observation".into(),
                route: "bind_action_to_local_preclinical_artifact".into(),
            });
        }
    }
    if matches!(
        request.workflow.disposition,
        LocalWorkflowDisposition::ApprovalRequired
    ) {
        add_barrier(MultimodalWorkflowBarrier {
            barrier_id: "global:workflow-approval".into(),
            study_id: "__global__".into(),
            action_id: "__global__".into(),
            kind: MultimodalWorkflowBarrierKind::WorkflowApproval,
            missing_modality: None,
            missing_model_system: None,
            severity_milli: 1_000,
            rationale: "the source local workflow has approval-required actions".into(),
            route: "obtain_scoped_authorization_before_workflow_admission".into(),
        });
    } else if matches!(
        request.workflow.disposition,
        LocalWorkflowDisposition::Partial | LocalWorkflowDisposition::Blocked
    ) {
        add_barrier(MultimodalWorkflowBarrier {
            barrier_id: "global:workflow-incomplete".into(),
            study_id: "__global__".into(),
            action_id: "__global__".into(),
            kind: MultimodalWorkflowBarrierKind::WorkflowIncomplete,
            missing_modality: None,
            missing_model_system: None,
            severity_milli: 1_000,
            rationale: "the source local workflow is not dependency-complete".into(),
            route: "recompile_local_workflow_before_multimodal_admission".into(),
        });
    }

    barriers.sort_by(|left, right| left.barrier_id.cmp(&right.barrier_id));
    barriers.dedup_by(|left, right| left.barrier_id == right.barrier_id);
    if barriers.len() > request.max_barriers {
        return Err(MultimodalWorkflowError::InvalidRequest(
            "max_barriers is too small to report every blocking condition; increase the bound"
                .into(),
        ));
    }
    let barrier_order = barriers
        .iter()
        .map(|barrier| barrier.barrier_id.clone())
        .collect::<Vec<_>>();
    let missing_modality_order = missing_modalities.into_iter().collect::<Vec<_>>();
    let missing_model_system_order = missing_models.into_iter().collect::<Vec<_>>();
    let readiness_milli = study_readiness
        .iter()
        .map(|row| row.readiness_milli)
        .min()
        .unwrap_or(0);
    let has_approval = barriers
        .iter()
        .any(|barrier| barrier.kind == MultimodalWorkflowBarrierKind::WorkflowApproval);
    let has_hard_barrier = barriers.iter().any(|barrier| {
        matches!(
            barrier.kind,
            MultimodalWorkflowBarrierKind::NonExportableObservation
                | MultimodalWorkflowBarrierKind::WorkflowIncomplete
                | MultimodalWorkflowBarrierKind::UnalignedObservation
        )
    });
    let has_barrier = !barriers.is_empty();
    let selected_branch = if has_approval || has_hard_barrier {
        None
    } else if !has_barrier {
        Some("full_multimodal".into())
    } else if request.allow_degraded_branch
        && readiness_milli >= request.readiness_threshold_milli / 2
    {
        Some("degraded_coverage".into())
    } else {
        Some("acquire_missing_coverage".into())
    };
    let branch_order = vec![
        "acquire_missing_coverage".into(),
        "degraded_coverage".into(),
        "full_multimodal".into(),
    ];
    let mut branches = vec![
        MultimodalWorkflowBranch {
            branch_id: "acquire_missing_coverage".into(),
            kind: MultimodalWorkflowBranchKind::AcquireMissingCoverage,
            action_order: request.workflow.step_order.clone(),
            study_order: study_order.clone(),
            missing_modality_order: missing_modality_order.clone(),
            missing_model_system_order: missing_model_system_order.clone(),
            support_milli: 1_000_u16.saturating_sub(readiness_milli),
            selected: selected_branch.as_deref() == Some("acquire_missing_coverage"),
        },
        MultimodalWorkflowBranch {
            branch_id: "degraded_coverage".into(),
            kind: MultimodalWorkflowBranchKind::DegradedCoverage,
            action_order: request.workflow.step_order.clone(),
            study_order: study_order.clone(),
            missing_modality_order: missing_modality_order.clone(),
            missing_model_system_order: missing_model_system_order.clone(),
            support_milli: readiness_milli,
            selected: selected_branch.as_deref() == Some("degraded_coverage"),
        },
        MultimodalWorkflowBranch {
            branch_id: "full_multimodal".into(),
            kind: MultimodalWorkflowBranchKind::FullMultimodal,
            action_order: request.workflow.step_order.clone(),
            study_order: study_order.clone(),
            missing_modality_order: Vec::new(),
            missing_model_system_order: Vec::new(),
            support_milli: if has_barrier { 0 } else { readiness_milli },
            selected: selected_branch.as_deref() == Some("full_multimodal"),
        },
    ];
    branches.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    let disposition = if has_approval {
        MultimodalWorkflowDisposition::ApprovalRequired
    } else if selected_branch.as_deref() == Some("full_multimodal") {
        MultimodalWorkflowDisposition::Ready
    } else if selected_branch.as_deref() == Some("degraded_coverage") {
        MultimodalWorkflowDisposition::Degraded
    } else {
        MultimodalWorkflowDisposition::Blocked
    };
    let next_step = match disposition {
        MultimodalWorkflowDisposition::Ready => {
            "admit the full multimodal branch to downstream computation with study-level readiness gates"
        }
        MultimodalWorkflowDisposition::Degraded => {
            "run only the policy-approved degraded branch and carry missingness into interpretation"
        }
        MultimodalWorkflowDisposition::ApprovalRequired => {
            "obtain scoped approval before admitting any workflow action"
        }
        MultimodalWorkflowDisposition::Blocked => {
            "acquire or repair missing coverage, then recompile this synchronization gate"
        }
    };
    let omission_order = barriers
        .iter()
        .map(|barrier| format!("{}: {}", barrier.barrier_id, barrier.route))
        .collect::<Vec<_>>();
    let uncertainty = barriers
        .iter()
        .map(|barrier| format!("{}: {}", barrier.barrier_id, barrier.rationale))
        .collect::<Vec<_>>();
    let mut output = MultimodalKnowledgeWorkflow {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_workflow_digest: request.workflow.digest.clone(),
        study_order,
        action_order: request.workflow.step_order.clone(),
        study_readiness,
        barrier_order,
        barriers,
        branch_order,
        branches,
        selected_branch,
        omission_order,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| MultimodalWorkflowError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalWorkflowError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::workflow_compile::{
        LocalResearchWorkflow, LocalWorkflowDisposition, LocalWorkflowStep,
    };

    fn workflow(disposition: LocalWorkflowDisposition) -> LocalResearchWorkflow {
        let step = LocalWorkflowStep {
            step_id: "workflow-step:fusion".into(),
            action_id: "fusion".into(),
            claim_key: "egfr-invasion".into(),
            dependency_order: Vec::new(),
            wave: 0,
            retry_limit: 1,
            checkpoint_after: true,
            compensation_kind: "preserve_prior_knowledge".into(),
            expected_artifact_kind: "typed_evidence_artifact".into(),
        };
        let mut output = LocalResearchWorkflow {
            feature_id: super::super::workflow_compile::FEATURE_ID.into(),
            output_schema: super::super::workflow_compile::OUTPUT_SCHEMA.into(),
            objective: "glioma invasion".into(),
            source_plan_digest: ContentHash::of_value(&serde_json::json!({"plan": "test"}))
                .unwrap(),
            step_order: vec!["fusion".into()],
            parallel_waves: vec![vec!["fusion".into()]],
            checkpoint_order: vec!["fusion".into()],
            compensation_order: vec!["fusion".into()],
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

    fn observation(modality: GliomaModality) -> ModalityWorkflowObservation {
        ModalityWorkflowObservation {
            study_id: "study-a".into(),
            site_id: "site-a".into(),
            action_id: "fusion".into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            readiness_milli: 950,
            missingness_milli: 20,
            alignment_milli: 940,
            semantic_consistency_milli: 930,
            exportable: true,
            preclinical_only: true,
        }
    }

    fn request(observations: Vec<ModalityWorkflowObservation>) -> MultimodalWorkflowRequest {
        MultimodalWorkflowRequest {
            objective: "glioma invasion".into(),
            workflow: workflow(LocalWorkflowDisposition::Ready),
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
        }
    }

    #[test]
    fn selects_full_branch_when_multimodal_study_is_ready() {
        let output = compile_multimodal_knowledge_workflow(&request(vec![
            observation(GliomaModality::Imaging),
            observation(GliomaModality::Transcriptomics),
        ]))
        .expect("workflow");
        assert_eq!(output.disposition, MultimodalWorkflowDisposition::Ready);
        assert_eq!(output.selected_branch.as_deref(), Some("full_multimodal"));
        assert!(output.barriers.is_empty());
        output.validate().expect("digest validates");
    }

    #[test]
    fn routes_missing_modality_to_degraded_branch_with_explicit_barrier() {
        let output = compile_multimodal_knowledge_workflow(&request(vec![observation(
            GliomaModality::Imaging,
        )]))
        .expect("workflow");
        assert_eq!(output.disposition, MultimodalWorkflowDisposition::Degraded);
        assert_eq!(output.selected_branch.as_deref(), Some("degraded_coverage"));
        assert!(output
            .barriers
            .iter()
            .any(|barrier| barrier.kind == MultimodalWorkflowBarrierKind::MissingModality));
    }

    #[test]
    fn rejects_non_preclinical_observations_before_planning() {
        let mut observation = observation(GliomaModality::Imaging);
        observation.preclinical_only = false;
        assert!(matches!(
            compile_multimodal_knowledge_workflow(&request(vec![observation])),
            Err(MultimodalWorkflowError::InvalidRequest(_))
        ));
    }
}
