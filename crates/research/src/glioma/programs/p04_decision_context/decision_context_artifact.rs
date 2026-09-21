//! Portable typed decision-context artifact for preclinical glioma workflows.
//!
//! P04 normally produces an in-process [`DecisionContext`]. This feature materializes that
//! context as a bounded, consumer-declared contract that can be handed to a local agent,
//! workbench, SDK, or MCP client without reinterpreting scientific state. It carries executable
//! action metadata and explicit deferred/omitted/negative/uncertain partitions; it never upgrades
//! a planning candidate into an observed result and never moves raw research data.

use super::context_compiler::{DecisionAction, DecisionActionKind, DecisionContext};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, GliomaStageKind};
use bioprism_foundation::{AutonomyTier, Effect, PRECLINICAL_BOUNDARY};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContextArtifact1@1";
pub const MAX_ACTIONS: usize = 256;
pub const MAX_CLAIMS: usize = 4_096;
pub const MAX_PARTITION_ITEMS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextArtifactConsumer {
    LocalAgent,
    ResearcherWorkbench,
    RustSdk,
    PythonSdk,
    TypeScriptSdk,
    McpClient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextArtifactCompatibility {
    pub contract_version: String,
    pub consumer_order: Vec<DecisionContextArtifactConsumer>,
    pub semantic_loss_order: Vec<String>,
    pub local_raw_data_required: bool,
    pub clinical_decision_capable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextArtifactAction {
    pub action_id: String,
    pub claim_id: String,
    pub kind: DecisionActionKind,
    pub stage_kind: GliomaStageKind,
    pub target_modality: GliomaModality,
    pub target_model_system: GliomaModelSystem,
    pub priority_milli: u16,
    pub cost_units: u32,
    pub depends_on: Vec<String>,
    pub autonomy_tier: AutonomyTier,
    pub effects: BTreeSet<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextArtifactRequest {
    pub objective: String,
    pub study_id: String,
    pub epoch: u32,
    pub compatibility: DecisionContextArtifactCompatibility,
    pub context: DecisionContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextArtifact {
    pub feature_id: String,
    pub output_schema: String,
    pub artifact_id: String,
    pub objective: String,
    pub study_id: String,
    pub epoch: u32,
    pub boundary: String,
    pub source_context_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub actions: Vec<DecisionContextArtifactAction>,
    pub action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub compatibility: DecisionContextArtifactCompatibility,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextArtifactError {
    #[error("decision-context artifact request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-context source is invalid: {0}")]
    InvalidContext(String),
    #[error("decision-context artifact is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-context artifact digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DecisionContextArtifact) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "artifact_id": output.artifact_id,
        "objective": output.objective,
        "study_id": output.study_id,
        "epoch": output.epoch,
        "boundary": output.boundary,
        "source_context_digest": output.source_context_digest,
        "claim_order": output.claim_order,
        "actions": output.actions,
        "action_order": output.action_order,
        "deferred_action_order": output.deferred_action_order,
        "omission_order": output.omission_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "compatibility": output.compatibility,
    })
}

fn validate_compatibility(
    compatibility: &DecisionContextArtifactCompatibility,
) -> Result<(), DecisionContextArtifactError> {
    if compatibility.contract_version.trim().is_empty()
        || compatibility.consumer_order.is_empty()
        || !canonical(&compatibility.consumer_order)
        || !canonical(&compatibility.semantic_loss_order)
        || compatibility.local_raw_data_required
        || compatibility.clinical_decision_capable
    {
        return Err(DecisionContextArtifactError::InvalidRequest(
            "compatibility must declare ordered consumers, no semantic-loss omissions, local raw-data prohibition, and no clinical decision capability".into(),
        ));
    }
    Ok(())
}

fn action_from_context(action: &DecisionAction) -> DecisionContextArtifactAction {
    DecisionContextArtifactAction {
        action_id: action.action_id.clone(),
        claim_id: action.claim_id.clone(),
        kind: action.kind,
        stage_kind: action.candidate.stage_kind,
        target_modality: action.target_modality,
        target_model_system: action.target_model_system,
        priority_milli: action.priority_milli,
        cost_units: action.candidate.cost_units,
        depends_on: action.candidate.depends_on.clone(),
        autonomy_tier: action.candidate.autonomy_tier,
        effects: action.candidate.effects.clone(),
    }
}

impl DecisionContextArtifact {
    pub fn validate(&self) -> Result<(), DecisionContextArtifactError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.artifact_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.epoch == 0
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.source_context_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.action_order)
            || !canonical(&self.deferred_action_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.actions.len() != self.action_order.len()
            || self
                .actions
                .iter()
                .map(|action| action.action_id.clone())
                .collect::<Vec<_>>()
                != self.action_order
            || self.actions.len() > MAX_ACTIONS
            || self.claim_order.len() > MAX_CLAIMS
            || self.omission_order.len() > MAX_PARTITION_ITEMS
            || self.negative_evidence_order.len() > MAX_PARTITION_ITEMS
            || self.uncertainty_order.len() > MAX_PARTITION_ITEMS
            || self.digest.as_str().len() != 64
        {
            return Err(DecisionContextArtifactError::InvalidOutput(
                "identity, preclinical boundary, ordering, action partitions, bounds, or digest fields are invalid".into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        if self
            .action_order
            .iter()
            .any(|id| self.deferred_action_order.binary_search(id).is_ok())
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.claim_id.trim().is_empty()
                    || action.priority_milli > 1_000
                    || action.cost_units == 0
                    || action.effects.is_empty()
                    || !action
                        .depends_on
                        .iter()
                        .all(|dependency| action_ids.contains(dependency))
            })
        {
            return Err(DecisionContextArtifactError::InvalidOutput(
                "action identity, partition, dependency, priority, cost, or effect fields are invalid".into(),
            ));
        }
        validate_compatibility(&self.compatibility)?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionContextArtifactError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextArtifactError::Digest(
                "decision-context artifact digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Materialize a validated local decision context into a portable typed artifact.
pub fn materialize_glioma_decision_context_artifact(
    request: &DecisionContextArtifactRequest,
) -> Result<DecisionContextArtifact, DecisionContextArtifactError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.epoch == 0
        || request.objective != request.context.objective
        || request.context.claim_order.len() > MAX_CLAIMS
        || request.context.actions.len() > MAX_ACTIONS
    {
        return Err(DecisionContextArtifactError::InvalidRequest(
            "objective, study, epoch, source context binding, and bounded action/claim counts are required".into(),
        ));
    }
    validate_compatibility(&request.compatibility)?;
    request
        .context
        .validate()
        .map_err(|error| DecisionContextArtifactError::InvalidContext(error.to_string()))?;
    let mut actions = request
        .context
        .actions
        .iter()
        .map(action_from_context)
        .collect::<Vec<_>>();
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut output = DecisionContextArtifact {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        artifact_id: format!(
            "glioma-decision-context:{}:epoch-{}",
            request.study_id, request.epoch
        ),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        epoch: request.epoch,
        boundary: PRECLINICAL_BOUNDARY.into(),
        source_context_digest: request.context.digest.clone(),
        claim_order: request.context.claim_order.clone(),
        actions,
        action_order,
        deferred_action_order: request.context.deferred_action_order.clone(),
        omission_order: request.context.omission_order.clone(),
        negative_evidence_order: request.context.negative_evidence_order.clone(),
        uncertainty_order: request.context.uncertainty_order.clone(),
        compatibility: request.compatibility.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-context-artifact"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionContextArtifactError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::context_compiler::DecisionContextRequest;
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};
    use bioprism_ids::ContentHash;
    use std::collections::BTreeSet;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn context() -> DecisionContext {
        let evidence = EvidenceRecord {
            evidence_id: "e1".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "artifact-1".into(),
                content_hash: hash("artifact-1"),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Proteomics,
            model_system: Some(crate::glioma_engine::GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "package context".into(),
                required_modalities: BTreeSet::from([GliomaModality::Proteomics]),
                required_model_systems: BTreeSet::from([
                    crate::glioma_engine::GliomaModelSystem::Organoid,
                ]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[evidence],
        )
        .unwrap();
        crate::glioma::programs::p04_decision_context::compile_decision_context(
            &DecisionContextRequest {
                objective: "package context".into(),
                max_actions: 4,
                default_cost_units: 2,
            },
            &knowledge,
        )
        .unwrap()
    }

    fn request() -> DecisionContextArtifactRequest {
        DecisionContextArtifactRequest {
            objective: "package context".into(),
            study_id: "study-a".into(),
            epoch: 3,
            compatibility: DecisionContextArtifactCompatibility {
                contract_version: "glioma-context-compat/1".into(),
                consumer_order: vec![
                    DecisionContextArtifactConsumer::LocalAgent,
                    DecisionContextArtifactConsumer::ResearcherWorkbench,
                    DecisionContextArtifactConsumer::RustSdk,
                    DecisionContextArtifactConsumer::PythonSdk,
                    DecisionContextArtifactConsumer::TypeScriptSdk,
                    DecisionContextArtifactConsumer::McpClient,
                ],
                semantic_loss_order: Vec::new(),
                local_raw_data_required: false,
                clinical_decision_capable: false,
            },
            context: context(),
        }
    }

    fn reseal_context(context: &mut DecisionContext) {
        context.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": context.feature_id,
            "output_schema": context.output_schema,
            "objective": context.objective,
            "claim_order": context.claim_order,
            "actions": context.actions,
            "action_order": context.action_order,
            "deferred_action_order": context.deferred_action_order,
            "omission_order": context.omission_order,
            "negative_evidence_order": context.negative_evidence_order,
            "uncertainty_order": context.uncertainty_order,
            "disposition": context.disposition,
        }))
        .unwrap();
    }

    #[test]
    fn materializes_portable_artifact_with_stable_digest() {
        let first = materialize_glioma_decision_context_artifact(&request()).unwrap();
        let second = materialize_glioma_decision_context_artifact(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.output_schema, OUTPUT_SCHEMA);
        assert_eq!(first.boundary, PRECLINICAL_BOUNDARY);
        first.validate().unwrap();
    }

    #[test]
    fn preserves_deferred_and_negative_partitions() {
        let mut request = request();
        request.context.deferred_action_order = vec!["deferred-action".into()];
        request.context.negative_evidence_order = vec!["negative-evidence".into()];
        request.context.omission_order = vec!["missing-modality".into()];
        request.context.uncertainty_order = vec!["unknown-evidence".into()];
        reseal_context(&mut request.context);
        let artifact = materialize_glioma_decision_context_artifact(&request).unwrap();
        assert_eq!(artifact.deferred_action_order, vec!["deferred-action"]);
        assert_eq!(artifact.negative_evidence_order, vec!["negative-evidence"]);
        assert_eq!(artifact.omission_order, vec!["missing-modality"]);
        assert_eq!(artifact.uncertainty_order, vec!["unknown-evidence"]);
    }

    #[test]
    fn rejects_objective_or_clinical_compatibility_mismatch() {
        let mut artifact_request = request();
        artifact_request.objective = "different objective".into();
        assert!(matches!(
            materialize_glioma_decision_context_artifact(&artifact_request),
            Err(DecisionContextArtifactError::InvalidRequest(_))
        ));
        let mut artifact_request = request();
        artifact_request.compatibility.clinical_decision_capable = true;
        assert!(matches!(
            materialize_glioma_decision_context_artifact(&artifact_request),
            Err(DecisionContextArtifactError::InvalidRequest(_))
        ));
    }

    #[test]
    fn rejects_self_dependency_in_materialized_action() {
        let mut request = request();
        request.context.actions[0].candidate.depends_on =
            vec![request.context.actions[0].action_id.clone()];
        assert!(matches!(
            materialize_glioma_decision_context_artifact(&request),
            Err(DecisionContextArtifactError::InvalidContext(_))
        ));
    }
}
