//! Intent-to-DAG compilation for autonomous preclinical glioma computation.
//!
//! P09 already executes a typed computation graph and can adapt a candidate registry over
//! multiple rounds. This feature supplies the missing researcher-facing compiler: a scientist
//! declares the modalities and terminal analyses they need, and the compiler expands the request
//! into a deterministic, dependency-closed graph with explicit resource estimates. It never
//! reads raw payloads or invents observations; the resulting candidates are handed to the normal
//! P09 campaign executor and its institution-local worker seam.

use super::campaign::GliomaComputationCampaignRequest;
use super::execution::{
    ComputationCacheEntry, ComputationOperation, ComputationTask, MAX_INPUTS_PER_TASK,
};
use super::planning::ComputationCandidate;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationWorkflow1@1";
pub const MAX_MODALITIES: usize = 16;
pub const MAX_OPERATIONS: usize = 9;
pub const MAX_INPUT_ARTIFACTS: usize = 128;
pub const MAX_CANDIDATES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationWorkflowRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub modalities: Vec<GliomaModality>,
    pub operations: Vec<ComputationOperation>,
    pub input_artifact_ids: Vec<String>,
    pub budget_units: u64,
    pub duration_ticks: u64,
    pub max_tasks: usize,
    pub max_modalities: usize,
    pub min_modalities: usize,
    pub information_weight_milli: u16,
    pub uncertainty_weight_milli: u16,
    pub coverage_weight_milli: u16,
    pub cost_penalty_milli: u16,
    pub duration_penalty_milli: u16,
    pub require_deterministic: bool,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub allow_cache: bool,
    pub require_local_artifacts: bool,
    pub cache: Vec<ComputationCacheEntry>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationWorkflow {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub modality_order: Vec<GliomaModality>,
    pub operation_order: Vec<ComputationOperation>,
    pub input_artifact_ids: Vec<String>,
    pub candidates: Vec<ComputationCandidate>,
    pub requested_terminal_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub estimated_cost_units: u64,
    pub estimated_duration_ticks: u64,
    pub within_declared_resources: bool,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaComputationWorkflowError {
    #[error("computation workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation workflow graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("computation workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation workflow digest failed: {0}")]
    Digest(String),
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn operation_rank(operation: ComputationOperation) -> u8 {
    match operation {
        ComputationOperation::Ingest => 1,
        ComputationOperation::Normalize => 2,
        ComputationOperation::Register => 3,
        ComputationOperation::Segment => 4,
        ComputationOperation::Quantify => 5,
        ComputationOperation::Integrate => 6,
        ComputationOperation::ModelFit => 7,
        ComputationOperation::Validate => 8,
        ComputationOperation::Export => 9,
    }
}

fn operation_slug(operation: ComputationOperation) -> &'static str {
    match operation {
        ComputationOperation::Ingest => "ingest",
        ComputationOperation::Normalize => "normalize",
        ComputationOperation::Register => "register",
        ComputationOperation::Segment => "segment",
        ComputationOperation::Quantify => "quantify",
        ComputationOperation::Integrate => "integrate",
        ComputationOperation::ModelFit => "model_fit",
        ComputationOperation::Validate => "validate",
        ComputationOperation::Export => "export",
    }
}

fn modality_slug(modality: GliomaModality) -> &'static str {
    match modality {
        GliomaModality::Literature => "literature",
        GliomaModality::Histopathology => "histopathology",
        GliomaModality::Genomics => "genomics",
        GliomaModality::Transcriptomics => "transcriptomics",
        GliomaModality::Epigenomics => "epigenomics",
        GliomaModality::Proteomics => "proteomics",
        GliomaModality::Imaging => "imaging",
        GliomaModality::SingleCell => "single_cell",
        GliomaModality::Spatial => "spatial",
        GliomaModality::FunctionalPerturbation => "functional_perturbation",
        GliomaModality::OrganoidAssay => "organoid_assay",
        GliomaModality::AnimalModel => "animal_model",
        GliomaModality::Computational => "computational",
        GliomaModality::Instrument => "instrument",
        GliomaModality::Replication => "replication",
    }
}

fn task_shape(operation: ComputationOperation) -> (u64, u64, u32, u32, u32) {
    match operation {
        ComputationOperation::Ingest => (2, 10, 300, 200, 400),
        ComputationOperation::Normalize => (3, 12, 450, 300, 500),
        ComputationOperation::Register => (4, 16, 550, 450, 550),
        ComputationOperation::Segment => (5, 20, 700, 600, 650),
        ComputationOperation::Quantify => (4, 15, 650, 550, 700),
        ComputationOperation::Integrate => (8, 30, 900, 800, 900),
        ComputationOperation::ModelFit => (12, 45, 1_000, 950, 1_000),
        ComputationOperation::Validate => (7, 28, 850, 900, 800),
        ComputationOperation::Export => (2, 8, 250, 150, 300),
    }
}

fn modality_for(operation: ComputationOperation, modality: GliomaModality) -> GliomaModality {
    if matches!(
        operation,
        ComputationOperation::Integrate
            | ComputationOperation::ModelFit
            | ComputationOperation::Validate
            | ComputationOperation::Export
    ) {
        GliomaModality::Computational
    } else {
        modality
    }
}

fn candidate_id(
    study_id: &str,
    operation: ComputationOperation,
    modality: GliomaModality,
) -> String {
    if matches!(
        operation,
        ComputationOperation::Integrate
            | ComputationOperation::ModelFit
            | ComputationOperation::Validate
            | ComputationOperation::Export
    ) {
        format!("{study_id}:{}", operation_slug(operation))
    } else {
        format!(
            "{study_id}:{}:{}",
            operation_slug(operation),
            modality_slug(modality)
        )
    }
}

fn prerequisites(operation: ComputationOperation) -> Option<ComputationOperation> {
    match operation {
        ComputationOperation::Ingest => None,
        ComputationOperation::Normalize => Some(ComputationOperation::Ingest),
        ComputationOperation::Register => Some(ComputationOperation::Normalize),
        ComputationOperation::Segment => Some(ComputationOperation::Register),
        ComputationOperation::Quantify => Some(ComputationOperation::Segment),
        ComputationOperation::Integrate => Some(ComputationOperation::Quantify),
        ComputationOperation::ModelFit => Some(ComputationOperation::Integrate),
        ComputationOperation::Validate => Some(ComputationOperation::ModelFit),
        ComputationOperation::Export => Some(ComputationOperation::Validate),
    }
}

fn modality_specific(operation: ComputationOperation) -> bool {
    matches!(
        operation,
        ComputationOperation::Ingest
            | ComputationOperation::Normalize
            | ComputationOperation::Register
            | ComputationOperation::Segment
            | ComputationOperation::Quantify
    )
}

fn validate_request(
    request: &GliomaComputationWorkflowRequest,
) -> Result<(), GliomaComputationWorkflowError> {
    if request.objective.trim().is_empty()
        || !valid_identifier(&request.study_id)
        || request.modalities.is_empty()
        || request.modalities.len() > MAX_MODALITIES
        || request.operations.is_empty()
        || request.operations.len() > MAX_OPERATIONS
        || request.input_artifact_ids.is_empty()
        || request.input_artifact_ids.len() > MAX_INPUT_ARTIFACTS
        || request.budget_units == 0
        || request.duration_ticks == 0
        || request.max_tasks == 0
        || request.max_tasks > MAX_CANDIDATES
        || request.max_modalities == 0
        || request.max_modalities > MAX_MODALITIES
        || request.min_modalities > request.max_modalities
        || request.max_rounds == 0
        || request.replay_identity.as_str().len() != 64
    {
        return Err(GliomaComputationWorkflowError::InvalidRequest(
            "objective, safe study identity, bounded modalities/operations, local inputs, resources, and replay policy are required".into(),
        ));
    }
    let modalities = request.modalities.iter().collect::<BTreeSet<_>>();
    if modalities.len() != request.modalities.len() {
        return Err(GliomaComputationWorkflowError::InvalidRequest(
            "modalities must be unique".into(),
        ));
    }
    let operations = request.operations.iter().collect::<BTreeSet<_>>();
    if operations.len() != request.operations.len() {
        return Err(GliomaComputationWorkflowError::InvalidRequest(
            "operations must be unique".into(),
        ));
    }
    let mut input_ids = BTreeSet::new();
    if request
        .input_artifact_ids
        .iter()
        .any(|id| !valid_identifier(id) || !input_ids.insert(id.clone()))
    {
        return Err(GliomaComputationWorkflowError::InvalidRequest(
            "input artifact identifiers must be unique, bounded, and path-safe".into(),
        ));
    }
    if request.cache.len() > MAX_INPUT_ARTIFACTS
        || request.cache.iter().any(|entry| {
            !valid_identifier(&entry.task_id)
                || entry.replay_identity.as_str().len() != 64
                || entry.output_schema.trim().is_empty()
        })
    {
        return Err(GliomaComputationWorkflowError::InvalidRequest(
            "cache entries must be bounded and typed".into(),
        ));
    }
    Ok(())
}

fn operation_set(requested: &[ComputationOperation]) -> BTreeSet<ComputationOperation> {
    let mut operations = requested.iter().copied().collect::<BTreeSet<_>>();
    let mut frontier = requested.to_vec();
    while let Some(operation) = frontier.pop() {
        if let Some(prerequisite) = prerequisites(operation) {
            if operations.insert(prerequisite) {
                frontier.push(prerequisite);
            }
        }
    }
    operations
}

fn operations_in_order(operations: &BTreeSet<ComputationOperation>) -> Vec<ComputationOperation> {
    let mut ordered = operations.iter().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|operation| operation_rank(*operation));
    ordered
}

fn dependency_ids(
    study_id: &str,
    operation: ComputationOperation,
    modality: GliomaModality,
    modalities: &[GliomaModality],
    operations: &BTreeSet<ComputationOperation>,
) -> Vec<String> {
    match operation {
        ComputationOperation::Ingest => Vec::new(),
        ComputationOperation::Normalize
        | ComputationOperation::Register
        | ComputationOperation::Segment
        | ComputationOperation::Quantify => vec![candidate_id(
            study_id,
            prerequisites(operation).expect("modality operation has prerequisite"),
            modality,
        )],
        ComputationOperation::Integrate => modalities
            .iter()
            .filter(|_| operations.contains(&ComputationOperation::Quantify))
            .map(|modality| candidate_id(study_id, ComputationOperation::Quantify, *modality))
            .collect(),
        ComputationOperation::ModelFit
        | ComputationOperation::Validate
        | ComputationOperation::Export => vec![candidate_id(
            study_id,
            prerequisites(operation).expect("global operation has prerequisite"),
            GliomaModality::Computational,
        )],
    }
}

fn digest_input(workflow: &GliomaComputationWorkflow) -> serde_json::Value {
    serde_json::json!({
        "feature_id": workflow.feature_id,
        "output_schema": workflow.output_schema,
        "objective": workflow.objective,
        "study_id": workflow.study_id,
        "model_system": workflow.model_system,
        "replay_identity": workflow.replay_identity,
        "modality_order": workflow.modality_order,
        "operation_order": workflow.operation_order,
        "input_artifact_ids": workflow.input_artifact_ids,
        "candidates": workflow.candidates,
        "requested_terminal_order": workflow.requested_terminal_order,
        "dependency_order": workflow.dependency_order,
        "estimated_cost_units": workflow.estimated_cost_units,
        "estimated_duration_ticks": workflow.estimated_duration_ticks,
        "within_declared_resources": workflow.within_declared_resources,
        "uncertainty": workflow.uncertainty,
    })
}

impl GliomaComputationWorkflow {
    pub fn validate(&self) -> Result<(), GliomaComputationWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !valid_identifier(&self.study_id)
            || self.replay_identity.as_str().len() != 64
            || self.modality_order.is_empty()
            || self
                .modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.operation_order.is_empty()
            || self
                .operation_order
                .windows(2)
                .any(|pair| operation_rank(pair[0]) >= operation_rank(pair[1]))
            || self.input_artifact_ids.is_empty()
            || self.input_artifact_ids.len() > MAX_INPUT_ARTIFACTS
            || self
                .input_artifact_ids
                .iter()
                .any(|id| !valid_identifier(id))
            || self
                .input_artifact_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .requested_terminal_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || self.candidates.is_empty()
            || self.candidates.len() > MAX_CANDIDATES
            || self
                .dependency_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self.digest.as_str().len() != 64
        {
            return Err(GliomaComputationWorkflowError::InvalidOutput(
                "workflow identity, ordering, bounds, or digest fields are invalid".into(),
            ));
        }
        let candidate_map = self
            .candidates
            .iter()
            .map(|candidate| (candidate.candidate_id.clone(), candidate))
            .collect::<BTreeMap<_, _>>();
        if candidate_map.len() != self.candidates.len()
            || self.candidates.iter().any(|candidate| {
                candidate.candidate_id != candidate.task.task_id
                    || candidate.task.model_system != self.model_system
                    || candidate.task.output_schema.trim().is_empty()
                    || candidate.task.estimated_cost_units == 0
                    || candidate.task.estimated_duration_ticks == 0
                    || candidate.task.depends_on.iter().any(|dependency| {
                        dependency == &candidate.candidate_id
                            || !candidate_map.contains_key(dependency)
                    })
                    || candidate.task.input_artifact_ids.len() > MAX_INPUTS_PER_TASK
                    || candidate
                        .task
                        .input_artifact_ids
                        .iter()
                        .any(|id| !valid_identifier(id))
            })
        {
            return Err(GliomaComputationWorkflowError::InvalidGraph(
                "candidate identities, model binding, dependency closure, or input bounds are invalid".into(),
            ));
        }
        let all_ids = candidate_map.keys().cloned().collect::<BTreeSet<_>>();
        let ordered_ids = self
            .dependency_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if ordered_ids != all_ids
            || self.dependency_order.iter().enumerate().any(|(index, id)| {
                candidate_map[id].task.depends_on.iter().any(|dependency| {
                    self.dependency_order
                        .iter()
                        .position(|item| item == dependency)
                        .is_none_or(|dependency_index| dependency_index >= index)
                })
            })
            || self
                .requested_terminal_order
                .iter()
                .any(|id| !candidate_map.contains_key(id))
        {
            return Err(GliomaComputationWorkflowError::InvalidGraph(
                "dependency order or requested terminal set is not closed".into(),
            ));
        }
        let estimated_cost = self
            .candidates
            .iter()
            .map(|candidate| candidate.task.estimated_cost_units)
            .sum::<u64>();
        let estimated_duration = self
            .candidates
            .iter()
            .map(|candidate| candidate.task.estimated_duration_ticks)
            .sum::<u64>();
        if estimated_cost != self.estimated_cost_units
            || estimated_duration != self.estimated_duration_ticks
        {
            return Err(GliomaComputationWorkflowError::InvalidOutput(
                "resource estimates do not reconcile with candidates".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaComputationWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaComputationWorkflowError::InvalidOutput(
                "workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }

    /// Convert the compiled graph into the existing bounded campaign contract.
    pub fn campaign_request(
        &self,
        request: &GliomaComputationWorkflowRequest,
    ) -> Result<GliomaComputationCampaignRequest, GliomaComputationWorkflowError> {
        if request.objective != self.objective
            || request.study_id != self.study_id
            || request.replay_identity != self.replay_identity
        {
            return Err(GliomaComputationWorkflowError::InvalidRequest(
                "campaign request does not match the compiled workflow identity".into(),
            ));
        }
        Ok(GliomaComputationCampaignRequest {
            objective: self.objective.clone(),
            model_system: self.model_system,
            initial_candidates: self.candidates.clone(),
            budget_units: request.budget_units,
            duration_ticks: request.duration_ticks,
            max_rounds: request.max_rounds,
            max_retries: request.max_retries,
            max_tasks: request.max_tasks,
            max_modalities: request.max_modalities,
            min_modalities: request.min_modalities,
            information_weight_milli: request.information_weight_milli,
            uncertainty_weight_milli: request.uncertainty_weight_milli,
            coverage_weight_milli: request.coverage_weight_milli,
            cost_penalty_milli: request.cost_penalty_milli,
            duration_penalty_milli: request.duration_penalty_milli,
            require_deterministic: request.require_deterministic,
            allow_cache: request.allow_cache,
            require_local_artifacts: request.require_local_artifacts,
            cache: request.cache.clone(),
            replay_identity: request.replay_identity.clone(),
        })
    }
}

/// Expand a high-level glioma computation intent into a deterministic dependency-closed DAG.
pub fn compile_glioma_computation_workflow(
    request: &GliomaComputationWorkflowRequest,
) -> Result<GliomaComputationWorkflow, GliomaComputationWorkflowError> {
    validate_request(request)?;
    let modalities = request
        .modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let requested = request.operations.iter().copied().collect::<BTreeSet<_>>();
    let operations = operation_set(&request.operations);
    let operation_order = operations_in_order(&operations);
    let mut input_artifact_ids = request.input_artifact_ids.clone();
    input_artifact_ids.sort();
    let mut candidates = Vec::new();
    let mut requested_terminal_order = Vec::new();
    for operation in &operation_order {
        let operation_modalities = if modality_specific(*operation) {
            modalities.clone()
        } else {
            vec![GliomaModality::Computational]
        };
        for modality in operation_modalities {
            let id = candidate_id(&request.study_id, *operation, modality);
            let (cost, duration, information, uncertainty, coverage) = task_shape(*operation);
            let dependencies = dependency_ids(
                &request.study_id,
                *operation,
                modality,
                &modalities,
                &operations,
            );
            let input_artifact_ids = if *operation == ComputationOperation::Ingest {
                input_artifact_ids.clone()
            } else {
                dependencies
                    .iter()
                    .map(|dependency| format!("derived:{dependency}"))
                    .collect()
            };
            let task = ComputationTask {
                task_id: id.clone(),
                operation: *operation,
                model_system: request.model_system,
                depends_on: dependencies,
                input_artifact_ids,
                output_schema: format!(
                    "GliomaComputationTask1@1:{}:{}",
                    operation_slug(*operation),
                    modality_slug(modality)
                ),
                estimated_cost_units: cost,
                estimated_duration_ticks: duration,
                deterministic: true,
            };
            let candidate = ComputationCandidate {
                candidate_id: id.clone(),
                task,
                modality: modality_for(*operation, modality),
                information_gain_milli: information,
                uncertainty_reduction_milli: uncertainty,
                coverage_debt_milli: coverage,
                redundancy_group: format!(
                    "{}:{}",
                    operation_slug(*operation),
                    modality_slug(modality)
                ),
                required: requested.contains(operation),
            };
            if requested.contains(operation) {
                requested_terminal_order.push(id);
            }
            candidates.push(candidate);
        }
    }
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    requested_terminal_order.sort();
    if candidates.len() > MAX_CANDIDATES {
        return Err(GliomaComputationWorkflowError::InvalidGraph(
            "dependency-expanded workflow exceeds candidate bound".into(),
        ));
    }
    let candidate_map = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut dependency_order = Vec::new();
    for operation in &operation_order {
        let operation_modalities = if modality_specific(*operation) {
            modalities.clone()
        } else {
            vec![GliomaModality::Computational]
        };
        for modality in operation_modalities {
            let id = candidate_id(&request.study_id, *operation, modality);
            if candidate_map.contains_key(&id) {
                dependency_order.push(id);
            }
        }
    }
    let estimated_cost_units = candidates
        .iter()
        .map(|candidate| candidate.task.estimated_cost_units)
        .sum::<u64>();
    let estimated_duration_ticks = candidates
        .iter()
        .map(|candidate| candidate.task.estimated_duration_ticks)
        .sum::<u64>();
    let mut uncertainty = Vec::new();
    if estimated_cost_units > request.budget_units {
        uncertainty.push("declared-budget-below-workflow-estimate".into());
    }
    if estimated_duration_ticks > request.duration_ticks {
        uncertainty.push("declared-duration-below-workflow-estimate".into());
    }
    let mut workflow = GliomaComputationWorkflow {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        replay_identity: request.replay_identity.clone(),
        modality_order: modalities,
        operation_order,
        input_artifact_ids,
        candidates,
        requested_terminal_order,
        dependency_order,
        estimated_cost_units,
        estimated_duration_ticks,
        within_declared_resources: estimated_cost_units <= request.budget_units
            && estimated_duration_ticks <= request.duration_ticks,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-workflow"),
    };
    workflow.digest = ContentHash::of_value(&digest_input(&workflow))
        .map_err(|error| GliomaComputationWorkflowError::Digest(error.to_string()))?;
    workflow.validate()?;
    Ok(workflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> GliomaComputationWorkflowRequest {
        GliomaComputationWorkflowRequest {
            objective: "profile invasive organoid state across modalities".into(),
            study_id: "study-glioma-01".into(),
            model_system: GliomaModelSystem::Organoid,
            modalities: vec![GliomaModality::Transcriptomics, GliomaModality::Imaging],
            operations: vec![ComputationOperation::Export, ComputationOperation::ModelFit],
            input_artifact_ids: vec!["artifact-imaging".into(), "artifact-rna".into()],
            budget_units: 100,
            duration_ticks: 500,
            max_tasks: 64,
            max_modalities: 4,
            min_modalities: 2,
            information_weight_milli: 5,
            uncertainty_weight_milli: 4,
            coverage_weight_milli: 3,
            cost_penalty_milli: 1,
            duration_penalty_milli: 1,
            require_deterministic: true,
            max_rounds: 4,
            max_retries: 1,
            allow_cache: true,
            require_local_artifacts: true,
            cache: Vec::new(),
            replay_identity: ContentHash::of_bytes(b"workflow-replay"),
        }
    }

    #[test]
    fn compiler_closes_requested_terminals_and_replays_under_input_permutation() {
        let first = compile_glioma_computation_workflow(&request()).unwrap();
        let mut permuted = request();
        permuted.modalities.reverse();
        permuted.operations.reverse();
        permuted.input_artifact_ids.reverse();
        let second = compile_glioma_computation_workflow(&permuted).unwrap();
        assert_eq!(first, second);
        assert!(first
            .requested_terminal_order
            .iter()
            .all(|id| first.dependency_order.contains(id)));
        assert!(first
            .candidates
            .iter()
            .any(|candidate| candidate.task.operation == ComputationOperation::Ingest));
        assert!(first
            .candidates
            .iter()
            .any(|candidate| candidate.task.operation == ComputationOperation::Integrate));
        assert!(first.within_declared_resources);
    }

    #[test]
    fn compiler_preserves_resource_shortfall_without_claiming_readiness() {
        let mut request = request();
        request.budget_units = 1;
        request.duration_ticks = 1;
        let workflow = compile_glioma_computation_workflow(&request).unwrap();
        assert!(!workflow.within_declared_resources);
        assert!(workflow
            .uncertainty
            .contains(&"declared-budget-below-workflow-estimate".to_string()));
        assert!(workflow
            .uncertainty
            .contains(&"declared-duration-below-workflow-estimate".to_string()));
    }

    #[test]
    fn invalid_input_identifiers_are_rejected_before_graph_expansion() {
        let mut request = request();
        request.input_artifact_ids = vec!["../outside-store".into()];
        assert!(matches!(
            compile_glioma_computation_workflow(&request),
            Err(GliomaComputationWorkflowError::InvalidRequest(_))
        ));
    }
}
