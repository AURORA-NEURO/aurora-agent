//! End-to-end multimodal mechanism campaign for preclinical glioma research.
//!
//! This vertical joins measured modality vectors, pathway activity, and the bounded action
//! selector.  It is intentionally useful as an autonomous engine seam: a local provider can
//! supply observations and typed assay candidates, while this module decides whether the evidence
//! is strong enough to execute a next batch and preserves every gap when it is not.

use super::super::p03_multimodal_ingestion_qc::{
    analyze_glioma_multimodal_graph_fusion, GraphFusionAnalysis, GraphFusionDisposition,
    GraphFusionRequest, GraphFusionVector,
};
use super::super::p05_mechanism_exploration::{
    analyze_glioma_pathway_activity, PathwayActivityAnalysis, PathwayActivityDefinition,
    PathwayActivityDisposition, PathwayActivityObservation, PathwayActivityRequest,
};
use super::action_execution::{
    execute_glioma_action_portfolio, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionRequest, GliomaActionExecutor,
};
use crate::glioma_engine::{
    select_glioma_actions, GliomaActionCandidate, GliomaActionSelection, GliomaEngineError,
    GliomaModelSystem, GliomaSelectionConfig,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalMechanismCampaign1@1";
pub const EXECUTION_FEATURE_ID: &str = "GAF-GLIOMA-P07-F30";
pub const EXECUTION_OUTPUT_SCHEMA: &str = "GliomaMultimodalMechanismCampaignExecution1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalMechanismCampaignRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub graph: GraphFusionRequest,
    pub pathway: PathwayActivityRequest,
    pub selection: GliomaSelectionConfig,
    pub completed_action_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismCampaignDisposition {
    ReadyForExecution,
    PartialNeedsEvidence,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismCampaignExecutionDisposition {
    Completed,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalMechanismCampaignExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub campaign: MultimodalMechanismCampaign,
    pub execution: Option<ActionPortfolioExecution>,
    pub executed_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismCampaignExecutionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalMechanismCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub graph_analysis: GraphFusionAnalysis,
    pub pathway_analysis: PathwayActivityAnalysis,
    pub action_selection: GliomaActionSelection,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismCampaignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismCampaignError {
    #[error("mechanism campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism campaign graph analysis failed: {0}")]
    Graph(String),
    #[error("mechanism campaign pathway analysis failed: {0}")]
    Pathway(String),
    #[error("mechanism campaign action selection failed: {0}")]
    Selection(String),
    #[error("mechanism campaign execution failed: {0}")]
    Execution(String),
    #[error("mechanism campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism campaign digest failed: {0}")]
    Digest(String),
}

fn ordered(items: &[String]) -> bool {
    items.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultimodalMechanismCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "graph_analysis": output.graph_analysis,
        "pathway_analysis": output.pathway_analysis,
        "action_selection": output.action_selection,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn execution_digest_input(output: &MultimodalMechanismCampaignExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "campaign": output.campaign,
        "execution": output.execution,
        "executed_order": output.executed_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MultimodalMechanismCampaign {
    pub fn validate(&self) -> Result<(), MechanismCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.study_id.trim().is_empty()
            || !ordered(&self.next_action_order)
            || !ordered(&self.negative_evidence)
            || !ordered(&self.uncertainty)
            || self.next_action_order != self.action_selection.selected_order
            || self.next_action_order.iter().any(|id| id.trim().is_empty())
        {
            return Err(MechanismCampaignError::InvalidOutput(
                "identity, ordering, action selection, or campaign partitions are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(MechanismCampaignError::InvalidOutput(
                "digest is not bound to mechanism campaign".into(),
            ));
        }
        Ok(())
    }
}

impl MultimodalMechanismCampaignExecution {
    pub fn validate(&self) -> Result<(), MechanismCampaignError> {
        if self.feature_id != EXECUTION_FEATURE_ID
            || self.output_schema != EXECUTION_OUTPUT_SCHEMA
            || self
                .executed_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .execution
                .as_ref()
                .is_some_and(|execution| self.executed_order != execution.completed_order)
        {
            return Err(MechanismCampaignError::InvalidOutput(
                "execution identity, ordering, or completed partition is invalid".into(),
            ));
        }
        self.campaign.validate()?;
        let expected = ContentHash::of_value(&execution_digest_input(self))
            .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(MechanismCampaignError::InvalidOutput(
                "digest is not bound to mechanism campaign execution".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MultimodalMechanismCampaignRequest,
) -> Result<(), MechanismCampaignError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.graph.study_id != request.study_id
        || request.pathway.study_id != request.study_id
        || request.graph.model_system != request.model_system
        || request.pathway.model_system != request.model_system
        || !ordered(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(MechanismCampaignError::InvalidRequest(
            "objective, study/model bindings, or completed action ordering are invalid".into(),
        ));
    }
    Ok(())
}

/// Run the local multimodal-to-mechanism-to-action vertical. The returned action selection is a
/// plan only; the caller must pass it through the existing protocol/instrument executor and its
/// approval gates before any effect occurs.
pub fn execute_glioma_multimodal_mechanism_campaign(
    request: &MultimodalMechanismCampaignRequest,
    graph_vectors: &[GraphFusionVector],
    pathway_definitions: &[PathwayActivityDefinition],
    pathway_observations: &[PathwayActivityObservation],
    candidates: &[GliomaActionCandidate],
) -> Result<MultimodalMechanismCampaign, MechanismCampaignError> {
    validate_request(request)?;
    let graph_analysis = analyze_glioma_multimodal_graph_fusion(&request.graph, graph_vectors)
        .map_err(|e| MechanismCampaignError::Graph(e.to_string()))?;
    let pathway_analysis = analyze_glioma_pathway_activity(
        &request.pathway,
        pathway_definitions,
        pathway_observations,
    )
    .map_err(|e| MechanismCampaignError::Pathway(e.to_string()))?;
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let action_selection = select_glioma_actions(candidates, &completed, &request.selection)
        .map_err(|e: GliomaEngineError| MechanismCampaignError::Selection(e.to_string()))?;
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    negative.extend(graph_analysis.negative_evidence.iter().cloned());
    negative.extend(pathway_analysis.negative_evidence.iter().cloned());
    uncertainty.extend(graph_analysis.uncertainty.iter().cloned());
    uncertainty.extend(pathway_analysis.uncertainty.iter().cloned());
    if graph_analysis.disposition != GraphFusionDisposition::Qualified {
        uncertainty.insert("multimodal graph evidence is not fully qualified".into());
    }
    if pathway_analysis.disposition != PathwayActivityDisposition::Qualified {
        uncertainty.insert("pathway activity evidence is not fully qualified".into());
    }
    if action_selection.selected_order.is_empty() {
        negative.insert(
            "no safe dependency-complete action was selected for this campaign cycle".into(),
        );
    }
    let disposition = if action_selection.selected_order.is_empty()
        || graph_analysis.disposition == GraphFusionDisposition::Unresolved
        || pathway_analysis.disposition == PathwayActivityDisposition::Unresolved
    {
        MechanismCampaignDisposition::Unresolved
    } else if graph_analysis.disposition == GraphFusionDisposition::Qualified
        && pathway_analysis.disposition == PathwayActivityDisposition::Qualified
        && negative.is_empty()
    {
        MechanismCampaignDisposition::ReadyForExecution
    } else {
        MechanismCampaignDisposition::PartialNeedsEvidence
    };
    let mut output = MultimodalMechanismCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        graph_analysis,
        pathway_analysis,
        next_action_order: action_selection.selected_order.clone(),
        action_selection,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Execute the selected multimodal mechanism portfolio through the caller-owned action worker.
/// Unresolved evidence never dispatches an action; successful execution still inherits the
/// portfolio executor's dependency, retry, artifact, and policy gates.
#[allow(clippy::too_many_arguments)]
pub fn execute_glioma_multimodal_mechanism_campaign_with_executor<E: GliomaActionExecutor>(
    request: &MultimodalMechanismCampaignRequest,
    graph_vectors: &[GraphFusionVector],
    pathway_definitions: &[PathwayActivityDefinition],
    pathway_observations: &[PathwayActivityObservation],
    candidates: &[GliomaActionCandidate],
    max_retries: u8,
    require_artifacts: bool,
    executor: &mut E,
) -> Result<MultimodalMechanismCampaignExecution, MechanismCampaignError> {
    let campaign = execute_glioma_multimodal_mechanism_campaign(
        request,
        graph_vectors,
        pathway_definitions,
        pathway_observations,
        candidates,
    )?;
    let mut negative = campaign
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut uncertainty = campaign
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if campaign.disposition == MechanismCampaignDisposition::Unresolved {
        negative.insert("campaign evidence or action safety gates blocked execution".into());
        let mut output = MultimodalMechanismCampaignExecution {
            feature_id: EXECUTION_FEATURE_ID.into(),
            output_schema: EXECUTION_OUTPUT_SCHEMA.into(),
            executed_order: Vec::new(),
            campaign,
            execution: None,
            negative_evidence: negative.into_iter().collect(),
            uncertainty: uncertainty.into_iter().collect(),
            disposition: MechanismCampaignExecutionDisposition::Blocked,
            digest: ContentHash::of_value(&serde_json::json!({}))
                .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?,
        };
        output.digest = ContentHash::of_value(&execution_digest_input(&output))
            .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    let completed_actions = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let execution = execute_glioma_action_portfolio(
        &ActionPortfolioExecutionRequest {
            candidates: candidates.to_vec(),
            completed_actions,
            selection: request.selection.clone(),
            max_retries,
            require_artifacts,
        },
        executor,
    )
    .map_err(|e| MechanismCampaignError::Execution(e.to_string()))?;
    negative.extend(execution.negative_evidence.iter().cloned());
    uncertainty.extend(execution.uncertainty.iter().cloned());
    let disposition = match execution.disposition {
        ActionPortfolioExecutionDisposition::Completed => {
            MechanismCampaignExecutionDisposition::Completed
        }
        ActionPortfolioExecutionDisposition::Partial
        | ActionPortfolioExecutionDisposition::Failed
        | ActionPortfolioExecutionDisposition::Blocked => {
            MechanismCampaignExecutionDisposition::Partial
        }
    };
    let mut output = MultimodalMechanismCampaignExecution {
        feature_id: EXECUTION_FEATURE_ID.into(),
        output_schema: EXECUTION_OUTPUT_SCHEMA.into(),
        executed_order: execution.completed_order.clone(),
        campaign,
        execution: Some(execution),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?,
    };
    output.digest = ContentHash::of_value(&execution_digest_input(&output))
        .map_err(|e| MechanismCampaignError::Digest(e.to_string()))?;
    output.validate()?;
    Ok(output)
}
